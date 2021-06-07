/// Configuration for domain checks to be performed on either the `Host`
/// or the `Origin` header.
#[derive(Debug)]
pub enum DomainCheck<Domain = String> {
	/// Allow any domain
    AllowAny,
    /// Allow a domain if it is one on the list
    AllowList(Vec<Domain>),
    /// Allow a domain if it is NOT on the list
    DenyList(Vec<Domain>),
}

impl<Domain> Default for DomainCheck<Domain> {
    fn default() -> Self {
        DomainCheck::AllowAny
    }
}

impl<Domain> DomainCheck<Domain>
where
	Domain: AsRef<str>,
{
	/// Checks if a `domain` is allowed the handshake
    pub(crate) fn is_allowed(&self, domain: &[u8]) -> bool {
        match self {
            DomainCheck::AllowAny => true,
            DomainCheck::AllowList(list) => list.iter().any(|d| d.as_ref().as_bytes() == domain),
            DomainCheck::DenyList(list) => !list.iter().any(|d| d.as_ref().as_bytes() == domain),
        }
    }
}
