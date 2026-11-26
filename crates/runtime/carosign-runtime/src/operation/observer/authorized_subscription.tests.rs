use super::super::{AuthorizedSubscription, SubscriptionError, authorize};
use crate::operation::hub::EventHub;
use crate::ports::{AuthorityRefusal, CallerContext, PolicyAuthority};
use crate::ports::{BoxFuture, TransactionId};
use carosign_core::authorization::AuthorizationBoundary;
use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::ids::OperationId;
use carosign_core::ids::{AuthorityId, BootId, GrantId};

struct Policy {
    scope: GrantScope,
    revoked: bool,
}

#[path = "legacy_authority.tests.rs"]
mod legacy_authority;
use legacy_authority::Legacy;

impl PolicyAuthority for Policy {
    fn authorize(
        &self,
        _: &CallerContext,
        _: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        if self.revoked {
            return Box::pin(async { Err(AuthorityRefusal::Revoked) });
        }
        let grant = ValidatedGrant::issue_for_port(
            GrantId::from_bytes([1; 16]).unwrap(),
            AuthorityId::from_bytes([2; 16]).unwrap(),
            self.scope.clone(),
            BootId::from_bytes([3; 16]).unwrap(),
            100,
        );
        Box::pin(async move { Ok(grant) })
    }

    fn revalidate(
        &self,
        _: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        let _ = boundary;
        let revoked = self.revoked;
        Box::pin(async move {
            if revoked {
                Err(AuthorityRefusal::Revoked)
            } else {
                Ok(())
            }
        })
    }

    fn revalidate_caller<'a>(
        &'a self,
        _caller: &'a CallerContext,
        grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        self.revalidate(grant, boundary)
    }
}

#[test]
fn subscription_rejects_authority_without_caller_revalidation() {
    let operation = OperationId::from_bytes([4; 16]).unwrap();
    let caller =
        CallerContext::from_transport([0; 32], 1000, TransactionId::from_bytes([6; 16]).unwrap());
    let authority = Legacy(Policy {
        scope: GrantScope::Observe(operation),
        revoked: false,
    });
    let mut hub = EventHub::new(operation);
    assert!(matches!(
        futures_executor::block_on(AuthorizedSubscription::attach(
            &authority, &caller, &mut hub, 1, 0,
        )),
        Err(SubscriptionError::Authority(AuthorityRefusal::Unavailable))
    ));
}

#[test]
fn observation_requires_the_exact_operation_and_live_authority() {
    let operation = OperationId::from_bytes([4; 16]).unwrap();
    let other = OperationId::from_bytes([5; 16]).unwrap();
    let caller =
        CallerContext::from_transport([0; 32], 1000, TransactionId::from_bytes([6; 16]).unwrap());
    let run = |scope, revoked| {
        futures_executor::block_on(authorize(&Policy { scope, revoked }, &caller, operation))
    };
    assert!(run(GrantScope::Observe(operation), false).is_ok());
    assert_eq!(
        run(GrantScope::Observe(other), false).unwrap_err(),
        AuthorityRefusal::Denied
    );
    assert_eq!(
        run(GrantScope::Cancel(operation), false).unwrap_err(),
        AuthorityRefusal::Denied
    );
    assert_eq!(
        run(GrantScope::Observe(operation), true).unwrap_err(),
        AuthorityRefusal::Revoked
    );
}

#[test]
fn subscription_revalidates_and_cannot_follow_reused_identifier() {
    use crate::operation::events::EventKind;
    use carosign_engine::OperationPhase;
    let operation = OperationId::from_bytes([4; 16]).unwrap();
    let caller =
        CallerContext::from_transport([0; 32], 1000, TransactionId::from_bytes([6; 16]).unwrap());
    let valid = Policy {
        scope: GrantScope::Observe(operation),
        revoked: false,
    };
    let revoked = Policy {
        scope: GrantScope::Observe(operation),
        revoked: true,
    };
    let mut hub = EventHub::new(operation);
    let old = futures_executor::block_on(AuthorizedSubscription::attach(
        &valid, &caller, &mut hub, 1, 0,
    ))
    .unwrap();
    let mut replacement_hub = EventHub::new(operation);
    let replacement = futures_executor::block_on(AuthorizedSubscription::attach(
        &valid,
        &caller,
        &mut replacement_hub,
        1,
        0,
    ))
    .unwrap();
    assert_eq!(
        futures_executor::block_on(old.next(&valid, &mut replacement_hub)),
        Err(SubscriptionError::Detached)
    );
    assert!(replacement.detach(&mut replacement_hub));
    hub.publish(OperationPhase::Running, EventKind::StateChanged)
        .unwrap();
    assert_eq!(
        futures_executor::block_on(old.next(&valid, &mut hub))
            .unwrap()
            .unwrap()
            .sequence,
        1
    );
    hub.publish(OperationPhase::Terminal, EventKind::Terminal)
        .unwrap();
    assert_eq!(
        futures_executor::block_on(old.next(&valid, &mut hub)),
        Err(SubscriptionError::TerminalNeedsStorage)
    );
    assert_eq!(
        futures_executor::block_on(old.next(&revoked, &mut hub)),
        Err(SubscriptionError::Authority(AuthorityRefusal::Revoked))
    );
    let new = futures_executor::block_on(AuthorizedSubscription::attach(
        &valid, &caller, &mut hub, 1, 2,
    ))
    .unwrap();
    assert_eq!(
        futures_executor::block_on(old.next(&valid, &mut hub)),
        Err(SubscriptionError::Detached)
    );
    assert!(!old.detach(&mut hub));
    assert!(new.detach(&mut hub));
}
