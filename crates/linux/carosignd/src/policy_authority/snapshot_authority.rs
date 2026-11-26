use super::{
    Arc, AuthorityId, AuthorityRefusal, AuthorizationBoundary, BoxFuture, CallerContext,
    FileIdentity, FilePolicy, GrantId, GrantScope, HashMap, IdentityAuthority, IdentityError,
    Issued, MAX_GRANTS, Mutex, Path, PathBuf, Policy, PolicyAuthority, PolicyChangeWatch,
    PolicyWatchState, ProtectedPolicySnapshot, RequestedSubject, State, SubjectRef,
    SubjectSelector, TimeSource, ValidatedGrant, VerificationIntent, account_uid, policy,
    same_grant,
};

/// Each copy owns one picture; after the recall, it is not updated.
pub(crate) struct SnapshotAuthority {
    pub(super) policy: Policy,
    pub(super) path: PathBuf,
    pub(super) authority: AuthorityId,
    pub(super) clock: Arc<dyn TimeSource>,
    pub(super) state: Mutex<State>,
}

impl SnapshotAuthority {
    pub(crate) fn is_current(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|mut state| Self::check(&mut state).is_ok())
    }
    pub(crate) fn authorize_inventory(&self, uid: u32) -> Result<(), AuthorityRefusal> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AuthorityRefusal::Unavailable)?;
        Self::check(&mut state)?;
        if self
            .policy
            .permits(uid, u64::from(uid), policy::Action::Inventory)
        {
            Ok(())
        } else {
            Err(AuthorityRefusal::Denied)
        }
    }

    pub(crate) fn authorize_operation_action(
        &self,
        uid: u32,
        subject: Option<u32>,
        action: policy::Action,
    ) -> Result<(), AuthorityRefusal> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AuthorityRefusal::Unavailable)?;
        Self::check(&mut state)?;
        let permitted = match subject {
            Some(subject) => {
                let subject = u64::from(subject);
                self.policy.permits(uid, subject, action)
                    && self
                        .policy
                        .lifetime_s(uid, subject)
                        .is_some_and(|lifetime| lifetime > 0)
            }
            None => self.policy.permits_for_any_subject(uid, action),
        };
        permitted.then_some(()).ok_or(AuthorityRefusal::Denied)
    }

    /// Allows a system account for subsequent separate validation of rights to change; the
    /// Verify rule is not required here and does not issue Enroll.
    pub(crate) fn resolve_mutation_subject(
        &self,
        caller: &CallerContext,
        selector: &carosign_protocol::carosign::common::v1::SubjectSelector,
    ) -> Result<SubjectRef, IdentityError> {
        use carosign_protocol::carosign::common::v1::subject_selector::Selector;
        let uid = match selector.selector.as_ref() {
            Some(Selector::CurrentSubject(true)) => caller.uid(),
            Some(Selector::AccountHint(name)) => account_uid(name)?,
            _ => return Err(IdentityError::UnknownSubject),
        };
        if caller.uid() != 0 && caller.uid() != uid {
            return Err(IdentityError::NotPermitted);
        }
        let mut state = self.state.lock().map_err(|_| IdentityError::Unavailable)?;
        Self::check(&mut state).map_err(|_| IdentityError::Unavailable)?;
        Ok(SubjectRef {
            authority: self.authority,
            subject: policy::subject_of(u64::from(uid)),
        })
    }
    #[cfg(test)]
    pub(crate) fn fixture(path: &Path, authority: AuthorityId, clock: Arc<dyn TimeSource>) -> Self {
        let text = std::fs::read_to_string(path).unwrap();
        Self {
            policy: policy::parse(&text).unwrap(),
            path: path.to_path_buf(),
            authority,
            clock,
            state: Mutex::new(State {
                watch: PolicyChangeWatch::new(path).unwrap(),
                issued: HashMap::new(),
                invalid: false,
            }),
        }
    }
    pub(crate) fn open(
        path: &Path,
        authority: AuthorityId,
        clock: Arc<dyn TimeSource>,
    ) -> Result<Self, AuthorityRefusal> {
        let snapshot =
            ProtectedPolicySnapshot::open(path).map_err(|_| AuthorityRefusal::Unavailable)?;
        let (bytes, mut watch) = snapshot.into_parts();
        let text = std::str::from_utf8(&bytes).map_err(|_| AuthorityRefusal::Unavailable)?;
        let policy = policy::parse(text).map_err(|_| AuthorityRefusal::Unavailable)?;
        if watch.check().ok() != Some(PolicyWatchState::Unchanged) {
            return Err(AuthorityRefusal::Unavailable);
        }
        Ok(Self {
            policy,
            path: path.to_path_buf(),
            authority,
            clock,
            state: Mutex::new(State {
                watch,
                issued: HashMap::new(),
                invalid: false,
            }),
        })
    }

    /// Creates a separate observation for the owner of the operation, and the main
    /// observation is kept locked before and after the installation of a new one: if the path
    /// changed during the installation, the observation is not issued.
    /// Surveillance itself does not confirm identity or permission.
    pub(crate) fn operation_watch(&self) -> Result<PolicyChangeWatch, AuthorityRefusal> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AuthorityRefusal::Unavailable)?;
        Self::check(&mut state)?;
        let mut watch =
            PolicyChangeWatch::new(&self.path).map_err(|_| AuthorityRefusal::Unavailable)?;
        Self::check(&mut state)?;
        if watch.check().ok() != Some(PolicyWatchState::Unchanged) {
            state.invalid = true;
            state.issued.clear();
            return Err(AuthorityRefusal::Revoked);
        }
        Ok(watch)
    }

    /// Provides surveillance only to the owner of the grant already issued by this photo.
    /// Checks on both sides of the surveillance installation do not replace the system
    /// identification of the caller: it must be performed by transport.
    pub(crate) fn operation_watch_for(
        &self,
        caller: &CallerContext,
        grant: &ValidatedGrant,
    ) -> Result<PolicyChangeWatch, AuthorityRefusal> {
        self.revalidate_for(caller, grant)?;
        let watch = self.operation_watch()?;
        self.revalidate_for(caller, grant)?;
        Ok(watch)
    }

    fn check(state: &mut State) -> Result<(), AuthorityRefusal> {
        if state.invalid || state.watch.check().ok() != Some(PolicyWatchState::Unchanged) {
            state.invalid = true;
            state.issued.clear();
            return Err(AuthorityRefusal::Revoked);
        }
        Ok(())
    }

    /// Re-check together with the original caller received from the transport.
    pub(crate) fn revalidate_for(
        &self,
        caller: &CallerContext,
        grant: &ValidatedGrant,
    ) -> Result<(), AuthorityRefusal> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AuthorityRefusal::Unavailable)?;
        Self::check(&mut state)?;
        let issued = state
            .issued
            .get(&grant.id())
            .ok_or(AuthorityRefusal::Revoked)?;
        if &issued.caller != caller || !same_grant(&issued.grant, grant) {
            return Err(AuthorityRefusal::Revoked);
        }
        let now = self.clock.now();
        if now.boot() != grant.boot()
            || self.clock.boot_id() != grant.boot()
            || now.instant_ns() >= grant.deadline_ns()
        {
            // Once an expired right must not come to life if a faulty time source later
            // communicates an earlier value.
            state.issued.remove(&grant.id());
            return Err(AuthorityRefusal::Expired);
        }
        // Checking the clock may coincide with a review, and before we issue a result, we
        // empty the observer's queue again, holding the same lock.
        Self::check(&mut state)?;
        Ok(())
    }
}

impl IdentityAuthority for SnapshotAuthority {
    fn resolve(
        &self,
        caller: &CallerContext,
        selector: &SubjectSelector,
    ) -> BoxFuture<'_, Result<SubjectRef, IdentityError>> {
        let caller = caller.clone();
        let current = matches!(selector, SubjectSelector::Current);
        Box::pin(async move {
            if !current {
                return Err(IdentityError::UnknownSubject);
            }
            {
                let mut state = self.state.lock().map_err(|_| IdentityError::Unavailable)?;
                Self::check(&mut state).map_err(|_| IdentityError::Unavailable)?;
            }
            let subject = FileIdentity::new(&self.policy, self.authority)
                .resolve(&caller, &SubjectSelector::Current)
                .await?;
            let mut state = self.state.lock().map_err(|_| IdentityError::Unavailable)?;
            Self::check(&mut state).map_err(|_| IdentityError::Unavailable)?;
            Ok(subject)
        })
    }

    fn resolve_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        intent: &'a VerificationIntent,
    ) -> BoxFuture<'a, Result<SubjectRef, IdentityError>> {
        Box::pin(async move {
            match &intent.subject {
                RequestedSubject::Current => self.resolve(caller, &SubjectSelector::Current).await,
                RequestedSubject::AccountHint(name) => {
                    let uid = account_uid(name)?;
                    self.resolve_account_uid(caller, uid)
                }
                RequestedSubject::Opaque(_) => Err(IdentityError::UnknownSubject),
            }
        })
    }
}

impl SnapshotAuthority {
    /// The name is allowed by the system, and the right to the received UID is allowed by the
    /// policy.
    pub(super) fn resolve_account_uid(
        &self,
        caller: &CallerContext,
        uid: u32,
    ) -> Result<SubjectRef, IdentityError> {
        if caller.uid() != 0 && caller.uid() != uid {
            return Err(IdentityError::NotPermitted);
        }
        let mut state = self.state.lock().map_err(|_| IdentityError::Unavailable)?;
        Self::check(&mut state).map_err(|_| IdentityError::Unavailable)?;
        let subject = SubjectRef {
            authority: self.authority,
            subject: policy::subject_of(u64::from(uid)),
        };
        if self
            .policy
            .verification_purpose(caller.uid(), subject, self.authority)
            .is_none()
        {
            return Err(IdentityError::NotPermitted);
        }
        Self::check(&mut state).map_err(|_| IdentityError::Unavailable)?;
        Ok(subject)
    }
}

impl PolicyAuthority for SnapshotAuthority {
    fn authorize_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        intent: &'a VerificationIntent,
        subject: SubjectRef,
    ) -> BoxFuture<'a, Result<ValidatedGrant, AuthorityRefusal>> {
        Box::pin(async move {
            // This source does not verify references to external authority.
            if !intent.authority_request_ref.is_empty() {
                return Err(AuthorityRefusal::Denied);
            }
            let purpose = self
                .policy
                .verification_purpose(caller.uid(), subject, self.authority)
                .ok_or(AuthorityRefusal::Denied)?;
            self.authorize(
                caller,
                GrantScope::Verify {
                    subject,
                    authenticator: intent.authenticator,
                    purpose,
                },
            )
            .await
        })
    }

    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        // The decision of the existing disassembler is only a preliminary check.
        // A predictable ID is not used as a system grant.
        let caller = caller.clone();
        Box::pin(async move {
            // A common port should not bypass the choice of purpose that performs the
            // authorize_verification: The goal allows a rule that does not call.
            if let GrantScope::Verify {
                subject, purpose, ..
            } = &scope
                && self
                    .policy
                    .verification_purpose(caller.uid(), *subject, self.authority)
                    != Some(*purpose)
            {
                return Err(AuthorityRefusal::Denied);
            }
            let file_policy =
                FilePolicy::with_clock(&self.policy, self.authority, self.clock.as_ref());
            let preliminary = file_policy.authorize(&caller, scope).await?;
            let mut state = self
                .state
                .lock()
                .map_err(|_| AuthorityRefusal::Unavailable)?;
            Self::check(&mut state)?;
            // Expired rights can no longer be confirmed and should not permanently occupy a
            // limited table of the current image.
            // Boot reconciliation is mandatory before deletion: the clock of another startup
            // does not allow you to compare the timing with the stored records.
            let prune_now = self.clock.now();
            if prune_now.boot() != self.clock.boot_id() || prune_now.boot() != preliminary.boot() {
                return Err(AuthorityRefusal::Unavailable);
            }
            Self::check(&mut state)?;
            state.issued.retain(|_, issued| {
                issued.grant.boot() != prune_now.boot()
                    || issued.grant.deadline_ns() > prune_now.instant_ns()
            });
            if state.issued.len() >= MAX_GRANTS {
                return Err(AuthorityRefusal::Unavailable);
            }
            let mut random = [0u8; 16];
            // getrandom does not read external paths and does not issue grants if refused.
            let n = unsafe { libc::getrandom(random.as_mut_ptr().cast(), random.len(), 0) };
            if n != random.len() as isize {
                return Err(AuthorityRefusal::Unavailable);
            }
            let id = GrantId::from_bytes(random).map_err(|_| AuthorityRefusal::Unavailable)?;
            if state.issued.contains_key(&id) {
                return Err(AuthorityRefusal::Unavailable);
            }
            // The source of chance and the reading of the clock may be delayed: the decision
            // before entering this site does not confirm the validity of the right
            // immediately before its publication in the table.
            let now = self.clock.now();
            if now.boot() != preliminary.boot()
                || self.clock.boot_id() != preliminary.boot()
                || now.instant_ns() >= preliminary.deadline_ns()
            {
                return Err(AuthorityRefusal::Expired);
            }
            Self::check(&mut state)?;
            let grant = ValidatedGrant::issue_for_port(
                id,
                preliminary.issuer(),
                preliminary.scope().clone(),
                preliminary.boot(),
                preliminary.deadline_ns(),
            );
            state.issued.insert(
                id,
                Issued {
                    caller,
                    grant: grant.clone(),
                },
            );
            Ok(grant)
        })
    }

    fn revalidate(
        &self,
        _grant: &ValidatedGrant,
        _boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        // The mere coincidence of the record with the grant does not confirm that the request
        // came from its owner. The old port does not transmit the caller, so you cannot allow
        // a critical crossing through it.
        Box::pin(async { Err(AuthorityRefusal::Unavailable) })
    }

    fn revalidate_caller<'a>(
        &'a self,
        caller: &'a CallerContext,
        grant: &'a ValidatedGrant,
        _boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        Box::pin(async move { self.revalidate_for(caller, grant) })
    }
}
