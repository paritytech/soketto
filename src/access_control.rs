use std::marker::PhantomData;

/// Access control policy used to check if a domain is allowed in
/// either the `Host` or `Origin` header.
pub trait Policy {
    /// Checks if a `domain` is allowed to handshake with us.
    fn is_allowed(&self, domain: &[u8]) -> bool;
}

/// Allow any domain, implements [`Policy`].
#[derive(Debug)]
pub struct AllowAny;

impl Policy for AllowAny {
    fn is_allowed(&self, _domain: &[u8]) -> bool {
        true
    }
}

/// Allow only domains from the list, implements [`Policy`].
#[derive(Debug)]
pub struct AllowList<List, Domain> {
    list: List,
    _marker: PhantomData<Domain>,
}

impl<List, Domain> AllowList<List, Domain> {
    pub fn new(list: List) -> Self {
        AllowList {
            list,
            _marker: PhantomData,
        }
    }
}

impl<List, Domain> Policy for AllowList<List, Domain>
where
    List: AsRef<[Domain]>,
    Domain: AsRef<str>,
{
    fn is_allowed(&self, domain: &[u8]) -> bool {
        self.list.as_ref().iter().any(|d| d.as_ref().as_bytes() == domain)
    }
}
