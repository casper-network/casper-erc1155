use blake2::{
    digest::{Update, VariableOutput},
    VarBlake2b,
};
use casper_engine_test_support::{
    DeployItemBuilder, ExecuteRequestBuilder, InMemoryWasmTestBuilder, WasmTestBuilder, ARG_AMOUNT,
    DEFAULT_ACCOUNT_ADDR, DEFAULT_PAYMENT, PRODUCTION_RUN_GENESIS_REQUEST,
};
use casper_erc1155::constants as consts;
use casper_types::{
    account::AccountHash,
    bytesrepr::{FromBytes, ToBytes},
    runtime_args,
    system::{handle_payment::ARG_TARGET, mint::ARG_ID},
    CLTyped, ContractHash, Key, PublicKey, RuntimeArgs, SecretKey, U256, U512,
};
use std::path::PathBuf;

use casper_execution_engine::{
    core::engine_state::ExecuteRequest, storage::global_state::in_memory::InMemoryGlobalState,
};
use rand::Rng;

const CONTRACT_ERC1155_TOKEN: &str = "erc1155_token.wasm";
const CONTRACT_KEY_NAME: &str = "erc1155_token_contract";

fn blake2b256(item_key_string: &[u8]) -> Box<[u8]> {
    let mut hasher = VarBlake2b::new(32).unwrap();
    hasher.update(item_key_string);
    hasher.finalize_boxed()
}

#[derive(Clone, Copy)]
pub struct Sender(pub AccountHash);

pub struct TestFixture {
    pub builder: InMemoryWasmTestBuilder,
    pub ali: AccountHash,
    pub bob: AccountHash,
    pub joe: AccountHash,
}
impl TestFixture {
    pub const URI: &'static str = "https://myuri-example.com";

    pub fn install_contract() -> TestFixture {
        let mut builder = InMemoryWasmTestBuilder::default();
        let ali_secret = SecretKey::ed25519_from_bytes([3u8; 32]).unwrap();
        let bob_secret = SecretKey::ed25519_from_bytes([6u8; 32]).unwrap();
        let joe_secret = SecretKey::ed25519_from_bytes([9u8; 32]).unwrap();

        let ali_pk: PublicKey = PublicKey::from(&ali_secret);
        let ali = ali_pk.to_account_hash();
        let bob_pk: PublicKey = PublicKey::from(&bob_secret);
        let bob = bob_pk.to_account_hash();
        let joe_pk: PublicKey = PublicKey::from(&joe_secret);
        let joe = joe_pk.to_account_hash();

        builder
            .run_genesis(&PRODUCTION_RUN_GENESIS_REQUEST)
            .commit();
        builder.exec(fund_account(&ali)).expect_success().commit();
        builder.exec(fund_account(&bob)).expect_success().commit();
        builder.exec(fund_account(&joe)).expect_success().commit();

        let session_code = PathBuf::from(CONTRACT_ERC1155_TOKEN);

        let session_args = runtime_args! {
            consts::URI_RUNTIME_ARG_NAME => TestFixture::URI,
        };

        let deploy_builder = DeployItemBuilder::new()
            .with_empty_payment_bytes(runtime_args! {ARG_AMOUNT => *DEFAULT_PAYMENT})
            .with_address(ali)
            .with_authorization_keys(&[ali])
            .with_session_code(session_code, session_args);

        let execute_request_builder =
            ExecuteRequestBuilder::from_deploy_item(deploy_builder.build());
        let exec = builder.exec(execute_request_builder.build());
        exec.expect_success().commit();

        TestFixture {
            builder,
            ali,
            bob,
            joe,
        }
    }

    fn contract_hash(&self) -> ContractHash {
        let account = self.builder.get_expected_account(self.ali);
        let keys = account.named_keys();

        let hash_addr = keys
            .get(CONTRACT_KEY_NAME)
            .expect("must have this entry in named keys")
            .into_hash()
            .unwrap();
        ContractHash::new(hash_addr)
    }

    fn query_contract<T: CLTyped + FromBytes>(&self, name: &str) -> Option<T> {
        match self.builder.query(
            None,
            Key::Account(self.ali),
            &[CONTRACT_KEY_NAME.to_string(), name.to_string()],
        ) {
            Err(_) => None,
            Ok(maybe_value) => {
                let value = maybe_value
                    .as_cl_value()
                    .expect("should be cl value.")
                    .clone()
                    .into_t()
                    .unwrap_or_else(|_| panic!("{} is not expected type.", name));
                Some(value)
            }
        }
    }

    fn call(&mut self, sender: Sender, method: &str, args: RuntimeArgs) {
        let Sender(address) = sender;
        let mut rng = rand::thread_rng();
        let deploy_builder = DeployItemBuilder::new()
            .with_empty_payment_bytes(runtime_args! {ARG_AMOUNT => *DEFAULT_PAYMENT})
            .with_stored_session_hash(self.contract_hash(), method, args)
            .with_address(address)
            .with_authorization_keys(&[address])
            .with_deploy_hash(rng.gen());
        let execute_request_builder =
            ExecuteRequestBuilder::from_deploy_item(deploy_builder.build());
        let exec = self.builder.exec(execute_request_builder.build());
        exec.expect_success().commit();
    }

    pub fn uri(&self) -> String {
        self.query_contract(consts::URI_RUNTIME_ARG_NAME).unwrap()
    }

    pub fn total_supply(&self, id: &str) -> Option<U256> {
        let item_key = format!("total_supply_{}", id);
        let key = Key::Hash(self.contract_hash().value());
        get_dictionary_value_from_key::<U256>(
            &self.builder,
            &key,
            consts::TOTAL_SUPPLY_KEY_NAME,
            &item_key,
        )
    }

    pub fn balance_of(&self, account: Key, id: &str) -> Option<U256> {
        let mut preimage = Vec::new();

        preimage.append(&mut id.to_bytes().unwrap());
        preimage.append(&mut account.to_bytes().unwrap());
        let key_bytes = blake2b256(&preimage);
        let balance_key = base64::encode(key_bytes);

        let key = Key::Hash(self.contract_hash().value());
        get_dictionary_value_from_key::<U256>(
            &self.builder,
            &key,
            consts::BALANCES_KEY_NAME,
            &balance_key,
        )
    }

    pub fn balance_of_batch(&self, accounts: Vec<Key>, ids: Vec<String>) -> Option<Vec<U256>> {
        let mut balances = Vec::new();
        let key = Key::Hash(self.contract_hash().value());
        for (i, _) in accounts.iter().enumerate() {
            let mut preimage = Vec::new();
            preimage.append(&mut ids[i].to_bytes().unwrap());
            preimage.append(&mut accounts[i].to_bytes().unwrap());
            let key_bytes = blake2b256(&preimage);
            let balance_key = base64::encode(key_bytes);
            let balance = get_dictionary_value_from_key::<U256>(
                &self.builder,
                &key,
                consts::BALANCES_KEY_NAME,
                &balance_key,
            );
            if let Some(balance) = balance {
                balances.push(balance);
            }
        }
        Some(balances)
    }

    pub fn set_approval_for_all(&mut self, operator: Key, approved: bool, sender: Sender) {
        self.call(
            sender,
            consts::SET_APPROVAL_FOR_ALL_ENTRY_POINT_NAME,
            runtime_args! {
                consts::OPERATOR_RUNTIME_ARG_NAME => operator,
                consts::APPROVED_RUNTIME_ARG_NAME => approved,
            },
        )
    }

    pub fn is_approval_for_all(&self, account: Key, operator: Key) -> Option<bool> {
        let mut preimage = Vec::new();
        preimage.append(&mut account.to_bytes().unwrap());
        preimage.append(&mut operator.to_bytes().unwrap());
        let key_bytes = blake2b256(&preimage);
        let approved_item_key = hex::encode(&key_bytes);

        let key = Key::Hash(self.contract_hash().value());
        get_dictionary_value_from_key::<bool>(
            &self.builder,
            &key,
            consts::OPERATORS_KEY_NAME,
            &approved_item_key,
        )
    }

    pub fn safe_transfer_from(
        &mut self,
        from: Key,
        to: Key,
        id: &str,
        amount: U256,
        sender: Sender,
    ) {
        self.call(
            sender,
            consts::SAFE_TRANSFER_FROM_ENTRY_POINT_NAME,
            runtime_args! {
                consts::FROM_RUNTIME_ARG_NAME => from,
                consts::RECIPIENT_RUNTIME_ARG_NAME => to,
                consts::TOKEN_ID_RUNTIME_ARG_NAME => id,
                consts::AMOUNT_RUNTIME_ARG_NAME => amount
            },
        );
    }

    pub fn safe_batch_transfer_from(
        &mut self,
        from: Key,
        to: Key,
        ids: Vec<String>,
        amounts: Vec<U256>,
        sender: Sender,
    ) {
        self.call(
            sender,
            consts::SAFE_BATCH_TRANSFER_FROM_ENTRY_POINT_NAME,
            runtime_args! {
                consts::FROM_RUNTIME_ARG_NAME => from,
                consts::RECIPIENT_RUNTIME_ARG_NAME => to,
                consts::TOKEN_IDS_RUNTIME_ARG_NAME => ids,
                consts::AMOUNTS_RUNTIME_ARG_NAME => amounts
            },
        );
    }

    pub fn mint(&mut self, to: Key, id: &str, amount: U256, sender: Sender) {
        self.call(
            sender,
            consts::MINT_ENTRY_POINT_NAME,
            runtime_args! {
                consts::RECIPIENT_RUNTIME_ARG_NAME => to,
                consts::TOKEN_ID_RUNTIME_ARG_NAME => id,
                consts::AMOUNT_RUNTIME_ARG_NAME => amount
            },
        );
    }

    pub fn burn(&mut self, owner: Key, id: &str, amount: U256, sender: Sender) {
        self.call(
            sender,
            consts::BURN_ENTRY_POINT_NAME,
            runtime_args! {
                consts::OWNER_RUNTIME_ARG_NAME => owner,
                consts::TOKEN_ID_RUNTIME_ARG_NAME => id,
                consts::AMOUNT_RUNTIME_ARG_NAME => amount
            },
        );
    }
}

fn fund_account(account: &AccountHash) -> ExecuteRequest {
    let deploy_item = DeployItemBuilder::new()
        .with_address(*DEFAULT_ACCOUNT_ADDR)
        .with_authorization_keys(&[*DEFAULT_ACCOUNT_ADDR])
        .with_empty_payment_bytes(runtime_args! {ARG_AMOUNT => *DEFAULT_PAYMENT})
        .with_transfer_args(runtime_args! {
            ARG_AMOUNT => U512::from(30_000_000_000_000_u64),
            ARG_TARGET => *account,
            ARG_ID => <Option::<u64>>::None
        })
        .with_deploy_hash([1; 32])
        .build();

    ExecuteRequestBuilder::from_deploy_item(deploy_item).build()
}

fn get_dictionary_value_from_key<T: CLTyped + FromBytes>(
    builder: &WasmTestBuilder<InMemoryGlobalState>,
    contract_key: &Key,
    dictionary_name: &str,
    dictionary_key: &str,
) -> Option<T> {
    let seed_uref = *builder
        .query(None, *contract_key, &[])
        .expect("must have contract")
        .as_contract()
        .expect("must convert contract")
        .named_keys()
        .get(dictionary_name)
        .expect("must have key")
        .as_uref()
        .expect("must convert to seed uref");
    let result = builder.query_dictionary_item(None, seed_uref, dictionary_key);

    if result.is_err() {
        return None;
    }

    let value = result
        .expect("should have dictionary value")
        .as_cl_value()
        .expect("T should be CLValue")
        .to_owned()
        .into_t()
        .unwrap();
    Some(value)
}
