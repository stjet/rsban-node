use rsnano_core::Account;

#[derive(Clone)]
pub struct Representative {
    account: Account,
    last_request: u64,
    last_response: u64,
}

impl Representative {
    pub fn new(account: Account) -> Self {
        Self {account, last_request: 0, last_response: 0}
    }

    pub fn account(&self) -> &Account{
        &self.account
    }

    pub fn last_request(&self) -> u64{
        self.last_request
    }

    pub fn set_last_request(&mut self, value: u64) {
        self.last_request = value
    }

    pub fn last_response(&mut self) -> u64{
        self.last_response
    }

    pub fn set_last_response(&mut self, value: u64) {
        self.last_response = value
    }
}
