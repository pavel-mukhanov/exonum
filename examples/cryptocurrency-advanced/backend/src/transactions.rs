// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Cryptocurrency transactions.

use exonum::{crypto::PublicKey, runtime::rust::TransactionContext};
use exonum_proto_derive::protobuf_convert;

use super::{proto, schema::Schema, CryptocurrencyService};

/// Error codes emitted by wallet transactions during execution.
#[derive(Debug, IntoExecutionError)]
pub enum Error {
    /// Wallet already exists.
    ///
    /// Can be emitted by `CreateWallet`.
    WalletAlreadyExists = 0,
    /// Sender doesn't exist.
    ///
    /// Can be emitted by `Transfer`.
    SenderNotFound = 1,
    /// Receiver doesn't exist.
    ///
    /// Can be emitted by `Transfer` or `Issue`.
    ReceiverNotFound = 2,
    /// Insufficient currency amount.
    ///
    /// Can be emitted by `Transfer`.
    InsufficientCurrencyAmount = 3,
    /// Sender are same as receiver.
    ///
    /// Can be emitted by 'Transfer`.
    SenderSameAsReceiver = 4,
}

/// Transfer `amount` of the currency from one wallet to another.
#[protobuf_convert(source = "proto::Transfer", serde_pb_convert)]
#[derive(Clone, Debug, BinaryValue, ObjectHash)]
pub struct Transfer {
    /// `PublicKey` of receiver's wallet.
    pub to: PublicKey,
    /// Amount of currency to transfer.
    pub amount: u64,
    /// Auxiliary number to guarantee [non-idempotence][idempotence] of transactions.
    ///
    /// [idempotence]: https://en.wikipedia.org/wiki/Idempotence
    pub seed: u64,
}

/// Issue `amount` of the currency to the `wallet`.
#[protobuf_convert(source = "proto::Issue")]
#[derive(Serialize, Deserialize, Clone, Debug, BinaryValue, ObjectHash)]
pub struct Issue {
    /// Issued amount of currency.
    pub amount: u64,
    /// Auxiliary number to guarantee [non-idempotence][idempotence] of transactions.
    ///
    /// [idempotence]: https://en.wikipedia.org/wiki/Idempotence
    pub seed: u64,
}

/// Create wallet with the given `name`.
#[protobuf_convert(source = "proto::CreateWallet")]
#[derive(Serialize, Deserialize, Clone, Debug, BinaryValue, ObjectHash)]
pub struct CreateWallet {
    /// Name of the new wallet.
    pub name: String,
}

/// Cryptocurrency service transactions.
#[exonum_service]
pub trait CryptocurrencyInterface {
    /// Transfers `amount` of the currency from one wallet to another.
    fn transfer(&self, ctx: TransactionContext, arg: Transfer) -> Result<(), Error>;
    /// Issues `amount` of the currency to the `wallet`.
    fn issue(&self, ctx: TransactionContext, arg: Issue) -> Result<(), Error>;
    /// Creates wallet with the given `name`.
    fn create_wallet(&self, ctx: TransactionContext, arg: CreateWallet) -> Result<(), Error>;
}

impl CryptocurrencyInterface for CryptocurrencyService {
    fn transfer(&self, context: TransactionContext, arg: Transfer) -> Result<(), Error> {
        let (tx_hash, from) = context
            .caller()
            .as_transaction()
            .expect("Wrong `Transfer` initiator");

        let mut schema = Schema::new(context.instance.name, context.fork());

        let to = arg.to;
        let amount = arg.amount;

        if from == to {
            return Err(Error::SenderSameAsReceiver);
        }

        let sender = schema.wallet(&from).ok_or(Error::SenderNotFound)?;

        let receiver = schema.wallet(&to).ok_or(Error::ReceiverNotFound)?;

        if sender.balance < amount {
            Err(Error::InsufficientCurrencyAmount)
        } else {
            schema.decrease_wallet_balance(sender, amount, tx_hash);
            schema.increase_wallet_balance(receiver, amount, tx_hash);
            Ok(())
        }
    }

    fn issue(&self, context: TransactionContext, arg: Issue) -> Result<(), Error> {
        let (tx_hash, from) = context
            .caller()
            .as_transaction()
            .expect("Wrong `Issue` initiator");

        let mut schema = Schema::new(context.instance.name, context.fork());

        if let Some(wallet) = schema.wallet(&from) {
            let amount = arg.amount;
            schema.increase_wallet_balance(wallet, amount, tx_hash);
            Ok(())
        } else {
            Err(Error::ReceiverNotFound)
        }
    }

    fn create_wallet(&self, context: TransactionContext, arg: CreateWallet) -> Result<(), Error> {
        let (tx_hash, from) = context
            .caller()
            .as_transaction()
            .expect("Wrong `CreateWallet` initiator");

        let mut schema = Schema::new(context.instance.name, context.fork());

        if schema.wallet(&from).is_none() {
            let name = &arg.name;
            schema.create_wallet(&from, name, tx_hash);
            Ok(())
        } else {
            Err(Error::WalletAlreadyExists)
        }
    }
}
