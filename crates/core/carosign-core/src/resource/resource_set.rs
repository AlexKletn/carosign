use super::InvalidResourceSet;
use super::ResourceClaim;
use core::fmt;

/// Maximum resources in one set.
pub const RESOURCE_SET_MAX: usize = 8;

/// The collection of resources in canonical order.
#[derive(Clone, PartialEq, Eq)]
pub struct ResourceSet(Vec<ResourceClaim>);

impl ResourceSet {
    /// Accepts the set by sorting and checking it.
    ///
    /// ⚠️ Repeating a resource is a mistake, even with the same mode: two modes per resource
    /// do not have the same meaning.
    pub fn new(mut claims: Vec<ResourceClaim>) -> Result<Self, InvalidResourceSet> {
        if claims.is_empty() || claims.len() > RESOURCE_SET_MAX {
            return Err(InvalidResourceSet);
        }
        claims.sort_by_key(|claim| claim.id);
        for pair in claims.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(InvalidResourceSet);
            }
        }
        Ok(Self(claims))
    }

    /// Requirements in canonical order.
    pub fn claims(&self) -> &[ResourceClaim] {
        &self.0
    }

    /// So many demands.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// It's never empty.
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl fmt::Debug for ResourceSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResourceSet({} ресурс(ов))", self.0.len())
    }
}
