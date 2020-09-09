use std::sync::Arc;
use crate::storage::vault::VaultAccessByFile;
use crate::structs::seed::{Seed, SeedSource, SeedRef};
use crate::structs::wallet::{Wallet, WalletEntry, PKType};
use uuid::Uuid;
use crate::storage::error::VaultError;
use hdpath::{StandardHDPath, AccountHDPath, PathValue};
use crate::blockchain::chains::{Blockchain, BlockchainType};
use bitcoin::util::bip32::{ExtendedPrivKey, ExtendedPubKey, DerivationPath};
use crate::blockchain::bitcoin::{AddressType, XPub};
use crate::sign::bitcoin::DEFAULT_SECP256K1;
use crate::structs::book::AddressRef;

pub struct AddBitcoinEntry {
    seeds: Arc<dyn VaultAccessByFile<Seed>>,
    wallets: Arc<dyn VaultAccessByFile<Wallet>>,
    wallet_id: Uuid,
}

fn get_address(blockchain: &Blockchain, address_type: AddressType, account: u32, seed: Vec<u8>) -> Result<XPub, VaultError> {
    let master = ExtendedPrivKey::new_master(blockchain.as_bitcoin_network(), seed.as_slice())
        .map_err(|_| VaultError::InvalidPrivateKey)?;
    if !PathValue::is_ok(account) {
        return Err(VaultError::PrivateKeyUnavailable)
    }
    let account = address_type.get_hd_path(account);
    let account_dp: DerivationPath = account.into();
    let xprv = master.derive_priv(&DEFAULT_SECP256K1, &account_dp)
        .map_err(|_| VaultError::PrivateKeyUnavailable)?;
    let xpub = ExtendedPubKey::from_private(&DEFAULT_SECP256K1, &xprv);
    Ok(XPub {
        value: xpub,
        address_type,
    })
}

impl AddBitcoinEntry {
    pub fn new(wallet_id: &Uuid,
               seeds: Arc<dyn VaultAccessByFile<Seed>>,
               wallets: Arc<dyn VaultAccessByFile<Wallet>>, ) -> AddBitcoinEntry {
        AddBitcoinEntry {
            wallet_id: wallet_id.clone(),
            seeds,
            wallets,
        }
    }

    pub fn seed_hd(
        &self,
        seed_id: Uuid,
        hd_path: StandardHDPath,
        blockchain: Blockchain,
        password: Option<String>,
    ) -> Result<usize, VaultError> {
        if blockchain.get_type() != BlockchainType::Bitcoin {
            return Err(VaultError::IncorrectBlockchainError)
        }
        let seed = self.seeds.get(seed_id)?;
        let address_type = AddressType::P2WPKH;
        let account = address_type.get_hd_path(hd_path.account());
        let address = match seed.source {
            SeedSource::Bytes(seed) => {
                if password.is_none() {
                    return Err(VaultError::PasswordRequired);
                }
                let seed = seed.decrypt(password.unwrap().as_str())?;
                let address = get_address(&blockchain, address_type, account.account(), seed)?;
                AddressRef::ExtendedPub(address)
            }
            SeedSource::Ledger(_) => {
                panic!("Ledger is not supported yet")
            }
        };

        let mut wallet = self.wallets.get(self.wallet_id.clone())?;
        let id = wallet.next_entry_id();
        wallet.entries.push(WalletEntry {
            id,
            blockchain,
            address: Some(address),
            key: PKType::SeedHd(SeedRef {
                seed_id: seed_id.clone(),
                hd_path: account.address_at(0, 0).expect("Generate first address for account"),
            }),
            ..WalletEntry::default()
        });
        wallet.entry_seq = id + 1;
        self.wallets.update(wallet.clone())?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;
    use crate::storage::vault::VaultStorage;
    use crate::mnemonic::{Mnemonic, Language};
    use std::convert::TryFrom;
    use crate::structs::wallet::ReservedPath;
    use std::str::FromStr;

    #[test]
    fn adds_seed_entry() {
        let tmp_dir = TempDir::new("emerald-vault-test").expect("Dir not created");
        let vault = VaultStorage::create(tmp_dir.path()).unwrap();
        let phrase = Mnemonic::try_from(
            Language::English,
            "avoid midnight couch purchase truth segment sauce claim spell spring smoke renew term stem solve",
        ).unwrap();
        let seed_id = vault.seeds().add(
            Seed {
                source: SeedSource::create_bytes(phrase.seed(None), "test").unwrap(),
                ..Default::default()
            }
        ).unwrap();
        let wallet_id = vault.wallets().add(Wallet {
            ..Default::default()
        }).unwrap();

        let entry_id = vault.add_bitcoin_entry(wallet_id.clone()).seed_hd(
            seed_id,
            StandardHDPath::from_str("m/84'/0'/3'/0/0").unwrap(),
            Blockchain::Bitcoin,
            Some("test".to_string()),
        ).expect("entry not created");

        let wallet = vault.wallets().get(wallet_id).unwrap();
        assert_eq!(
            vec![ReservedPath { seed_id, account_id: 3 }],
            wallet.reserved
        );
        assert_eq!(1, wallet.entries.len());
        let entry = &wallet.entries[0];

        assert_eq!(Blockchain::Bitcoin, entry.blockchain);
        assert!(entry.address.is_some());

        let address_ref = entry.address.as_ref().unwrap();
        match address_ref {
            AddressRef::ExtendedPub(xpub) => {
                assert_eq!(AddressType::P2WPKH, xpub.address_type);
            }
            _ => {
                panic!("not xpub")
            }
        }
    }
}
