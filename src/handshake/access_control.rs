// Copyright (c) 2021 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Access control policy trait and types.

use std::marker::PhantomData;

/// Access control policy used to check if a value is allowed in
/// a http header.
///
/// See [`Server::set_hosts`](`crate::handshake::Server::set_hosts`) and
/// [`Server::set_origins`](`crate::handshake::Server::set_origins`) for use.
pub trait Policy {
    /// Checks if a given `value` is allowed to handshake with us.
    fn is_allowed(&self, value: &[u8]) -> bool;
}

/// Allow any value, implements [`Policy`].
#[derive(Debug)]
pub struct AllowAny;

impl Policy for AllowAny {
    fn is_allowed(&self, _: &[u8]) -> bool {
        true
    }
}

/// Allow only values from the list, implements [`Policy`].
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
        self.list
            .as_ref()
            .iter()
            .any(|d| d.as_ref().as_bytes() == domain)
    }
}
