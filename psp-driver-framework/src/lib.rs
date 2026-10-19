//! The runner. Generic over the flow (`F: Flow`) and the database (`B: Backend`); it
//! knows nothing about either domain or SQL.
//!
//! The contract: `start` stores the bookmark before the first wait; `deliver` loads the
//! bookmark from the database, steps it, stores it again and journals, all inside one
//! transaction; `pending` rebuilds every waiting instance from the database after a
//! restart. A violation or a crash before commit leaves the database unchanged.

#![forbid(unsafe_code)]

use core::fmt::Debug;
use core::marker::PhantomData;
use psp_machine_framework::{Flow, InstanceId, Next, Stored};
use psp_store_framework::{Backend, JournalEntry, JournalRow, Persist, Status};

#[derive(Debug)]
pub enum DriverError<V: Debug, E: Debug> {
    Db(E),
    UnknownInstance(InstanceId),
    AlreadyFinished(InstanceId),
    /// The flow refused the response; the transaction was rolled back, nothing changed.
    Violation(V),
}

#[derive(Debug)]
pub enum Outcome<E, D> {
    /// The instance now waits on this interaction. Perform it, then `deliver` the response.
    Wait(E),
    Done(D),
}

pub struct Driver<F: Flow, B: Backend> {
    backend: B,
    _f: PhantomData<F>,
}

type Res<T, F, B> = Result<T, DriverError<<F as Flow>::Violation, <B as Backend>::Error>>;

impl<F: Persist<B>, B: Backend> Driver<F, B> {
    /// Wrap a backend. Creates the application's tables via `Persist::schema`.
    pub fn new(mut backend: B) -> Result<Self, B::Error> {
        backend.execute_ddl(<F as Persist<B>>::schema())?;
        Ok(Driver { backend, _f: PhantomData })
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Begin an instance: store the bookmark, return what it waits on.
    pub fn start(&mut self, s0: F::State) -> Res<(InstanceId, F::Event), F, B> {
        let mut tx = self.backend.begin().map_err(DriverError::Db)?;
        let id = B::insert_instance(&mut tx, F::NAME).map_err(DriverError::Db)?;
        Self::save(&mut tx, id, Stored::Waiting(s0))?;
        B::commit(tx).map_err(DriverError::Db)?;
        Ok((id, F::event(&s0)))
    }

    /// A response arrived: load, step, store, journal, commit.
    pub fn deliver(&mut self, id: InstanceId, r: F::Response) -> Res<Outcome<F::Event, F::Done>, F, B> {
        let mut tx = self.backend.begin().map_err(DriverError::Db)?;
        match B::instance(&mut tx, id).map_err(DriverError::Db)? {
            None => return Err(DriverError::UnknownInstance(id)),
            Some(info) if info.status == Status::Finished => return Err(DriverError::AlreadyFinished(id)),
            Some(info) => {
                let s = match Self::load(&mut tx, id)? {
                    Stored::Waiting(s) => s,
                    Stored::Finished(_) => return Err(DriverError::AlreadyFinished(id)),
                };
                let ev = F::event(&s);
                let entry = JournalEntry { at: format!("{s:?}"), event: format!("{ev:?}"), response: format!("{r:?}") };
                let next = F::step(s, r).map_err(DriverError::Violation)?; // `tx` dropped => rollback
                B::append_journal(&mut tx, id, info.seq, &entry).map_err(DriverError::Db)?;
                let outcome = match next {
                    Next::Wait(s1) => {
                        Self::save(&mut tx, id, Stored::Waiting(s1))?;
                        B::advance(&mut tx, id, info.seq + 1, Status::Waiting).map_err(DriverError::Db)?;
                        Outcome::Wait(F::event(&s1))
                    }
                    Next::Done(d) => {
                        Self::save(&mut tx, id, Stored::Finished(d))?;
                        B::advance(&mut tx, id, info.seq + 1, Status::Finished).map_err(DriverError::Db)?;
                        Outcome::Done(d)
                    }
                };
                B::commit(tx).map_err(DriverError::Db)?;
                Ok(outcome)
            }
        }
    }

    /// Crash recovery: every waiting instance of this flow and the interaction it waits
    /// on, rebuilt from the application's tables. Re-issue these on restart.
    pub fn pending(&mut self) -> Res<Vec<(InstanceId, F::Event)>, F, B> {
        let mut tx = self.backend.begin().map_err(DriverError::Db)?;
        let ids = B::waiting_instances(&mut tx, F::NAME).map_err(DriverError::Db)?;
        let mut out = Vec::new();
        for id in ids {
            if let Stored::Waiting(s) = Self::load(&mut tx, id)? {
                out.push((id, F::event(&s)));
            }
        }
        Ok(out)
    }

    pub fn journal(&mut self, id: InstanceId) -> Res<Vec<JournalRow>, F, B> {
        let mut tx = self.backend.begin().map_err(DriverError::Db)?;
        B::journal(&mut tx, id).map_err(DriverError::Db)
    }

    fn save(tx: &mut B::Tx<'_>, id: InstanceId, obj: Stored<F::State, F::Done>) -> Res<(), F, B> {
        <F as Persist<B>>::save(&obj, tx, id).map_err(DriverError::Db)
    }

    fn load(tx: &mut B::Tx<'_>, id: InstanceId) -> Res<Stored<F::State, F::Done>, F, B> {
        <F as Persist<B>>::load(tx, id).map_err(DriverError::Db)
    }
}
