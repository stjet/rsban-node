use std::collections::HashMap;

use super::Representative;
use rsnano_core::Account;

pub(crate) struct RepresentativeCollection {
    by_account: HashMap<Account, Representative>,
    by_channel_id: HashMap<usize, Vec<Account>>,
}
impl RepresentativeCollection {
    pub fn new() -> Self {
        Self {
            by_account: HashMap::new(),
            by_channel_id: HashMap::new(),
        }
    }

    pub fn add(&mut self, rep: Representative) {
        let account = *rep.account();
        let channel_id = rep.channel().as_channel().channel_id();

        let old = self.by_account.insert(account, rep);
        if old.is_some() {
            panic!("Tried to add representative twice")
        }
        let by_channel_id = self.by_channel_id.entry(channel_id).or_default();
        by_channel_id.push(account);
    }

    pub fn get(&self, account: &Account) -> Option<&Representative> {
        self.by_account.get(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_representative_by_account() {
        let mut reps = RepresentativeCollection::new();
        let rep = Representative::create_test_instance();
        let account = *rep.account();
        reps.add(rep);
        assert_eq!(reps.get(&account).unwrap().account(), &account);
        assert!(reps.get(&Account::from(1000)).is_none());
    }

    #[test]
    #[should_panic(expected = "Tried to add representative twice")]
    fn panics_if_account_already_added() {
        let mut reps = RepresentativeCollection::new();
        reps.add(Representative::create_test_instance());
        reps.add(Representative::create_test_instance());
    }
}
