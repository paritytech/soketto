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
pub struct AllowList<List, Value> {
    list: List,
    _marker: PhantomData<Value>,
}

impl<List, Value> AllowList<List, Value>
where
    List: AsRef<[Value]>,
    Value: AsRef<str>,
{
    /// Create a new list. The `list` source can be an array, a slice, or a `Vec` of
    /// `&str` slices or `String`s:
    ///
    /// ```rust
    /// use soketto::handshake::AllowList;
    ///
    /// let array = AllowList::new(["localhost"]);
    /// let slice = AllowList::new(&["localhost"]);
    /// let owned = AllowList::new(vec!["localhost".to_string()]);
    /// ```
    pub fn new(list: List) -> Self {
        AllowList {
            list,
            _marker: PhantomData,
        }
    }
}

impl<List, Value> Policy for AllowList<List, Value>
where
    List: AsRef<[Value]>,
    Value: AsRef<str>,
{
    fn is_allowed(&self, value: &[u8]) -> bool {
        self.list
            .as_ref()
            .iter()
            .any(|d| d.as_ref().as_bytes() == value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_any() {
        let policy = AllowAny;

        assert_eq!(true, policy.is_allowed(b"foobar"));
        assert_eq!(true, policy.is_allowed(b"localhost"));
    }

    #[test]
    fn allow_list() {
        let policy = AllowList::new(["localhost", "127.0.0.1"]);

        assert_eq!(true, policy.is_allowed(b"localhost"));
        assert_eq!(true, policy.is_allowed(b"127.0.0.1"));
        assert_eq!(false, policy.is_allowed(b"foobar"));
    }
}
