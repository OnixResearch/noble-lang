use super::*;

impl Session {
    pub fn admit(
        profile: crate::Profile,
        bytes: &[u8],
        binding: WitBinding<'_>,
    ) -> Result<Self, Error> {
        {
            let table = attempt!(profile.table_unchecked());
            if table.choreography_active {
                return Err(Error::Denied);
            }
            if table.counts() != (0, 0, 0) || !table.events().is_empty() {
                return Err(Error::Nonempty);
            }
        }
        let plan = attempt!(project(bytes));
        if binding != WitBinding::SERVICE {
            return Err(Error::Binding);
        }
        let (authority, publisher_id, subscriber_id) = {
            let mut table = attempt!(profile.table_unchecked());
            if table.choreography_active {
                return Err(Error::Denied);
            }
            if table.counts() != (0, 0, 0) || !table.events().is_empty() {
                return Err(Error::Nonempty);
            }
            // Reserve complete two-round work plus mandatory publication retirement,
            // independently of M7's per-operation owed-removal preflight.
            let limits = table.remaining_capacity();
            if limits.facets < 2 || limits.assertions < 1 {
                return Err(Error::Capacity);
            }
            if limits.interests < 2 || limits.events < 4 {
                return Err(Error::Capacity);
            }
            let authority = attempt!(NEXT_SESSION
                .fetch_update(
                    std::sync::atomic::Ordering::Relaxed,
                    std::sync::atomic::Ordering::Relaxed,
                    |next| next.checked_add(1),
                )
                .map_err(|_| Error::Capacity));
            let publisher_id = attempt!(table
                .open(
                    None,
                    noble_kernel::dataspace::Rights {
                        publish: true,
                        observe: false,
                    },
                )
                .map_err(crate::Error::from));
            let subscriber_id = match table.open(
                None,
                noble_kernel::dataspace::Rights {
                    publish: false,
                    observe: true,
                },
            ) {
                Ok(id) => id,
                Err(error) => {
                    attempt!(table.retire(publisher_id).map_err(crate::Error::from));
                    return Err(Error::Host(error.into()));
                }
            };
            table.choreography_active = true;
            (authority, publisher_id, subscriber_id)
        };
        let publisher = crate::Participant {
            table: profile.table.clone(),
            facet: publisher_id,
            live: true,
        };
        let subscriber = crate::Participant {
            table: profile.table.clone(),
            facet: subscriber_id,
            live: true,
        };
        Ok(Self {
            inner: std::sync::Mutex::new(Inner {
                profile,
                publisher,
                subscriber,
                plan,
                authority,
                cursor: 0,
                pending: None,
                guest_requests: 0,
                refused_requests: 0,
                protected_operations: 0,
            }),
        })
    }

    pub(super) fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, Error> {
        self.inner
            .lock()
            .map_err(|_| Error::Host(crate::Error::Poisoned))
    }

    pub fn token(&self, role: Role) -> Result<ParticipantToken, Error> {
        let inner = attempt!(self.lock());
        Ok(ParticipantToken {
            authority: inner.authority,
            role,
            facet: attempt!(inner.participant(role)).facet,
        })
    }

    pub fn snapshot(&self) -> Result<Snapshot, Error> {
        let inner = attempt!(self.lock());
        let table = attempt!(inner.profile.table_unchecked());
        Ok(Snapshot {
            cursor: inner.cursor,
            pending: inner.pending.is_some(),
            guest_requests: inner.guest_requests,
            refused_requests: inner.refused_requests,
            protected_operations: inner.protected_operations,
            counts: table.counts(),
            events_len: table.events().len(),
        })
    }
}
