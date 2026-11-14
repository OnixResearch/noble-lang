#![no_std]

pub fn first_match<'a, T, F>(items: &'a [T], mut predicate: F) -> Option<&'a T>
where
    F: FnMut(&T) -> bool,
{
    items.iter().find(|item| predicate(*item))
}
