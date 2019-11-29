use std::collections::{BTreeMap, HashMap};
use std::iter::{repeat, FromIterator};

use chain_core::init::{coin::Coin, params::SlashRatio};
use chain_core::state::tendermint::BlockHeight;
use chain_core::state::{
    account::CouncilNode, account::StakedState, account::StakedStateAddress,
    tendermint::TendermintValidatorAddress, tendermint::TendermintValidatorPubKey,
    tendermint::TendermintVotePower,
};
use parity_scale_codec::{Decode, Encode, Error, Input, Output};
use slab::Slab;

use crate::liveness::LivenessTracker;
use crate::slashing::SlashingSchedule;

fn remove_item<T: PartialEq>(xs: &mut Vec<T>, x: &T) -> Option<T> {
    xs.iter()
        .position(|v| v == x)
        .map(move |i| xs.swap_remove(i))
}

fn update_power_index(
    index: &mut BTreeMap<TendermintVotePower, Vec<usize>>,
    id: usize,
    old_power: TendermintVotePower,
    new_power: TendermintVotePower,
) {
    // remove old
    remove_item(index.get_mut(&old_power).unwrap(), &id).unwrap();
    // insert new
    index
        .entry(new_power)
        .and_modify(|xs| xs.push(id))
        .or_insert_with(|| vec![id]);
}

pub type ValidatorCandidate = (TendermintValidatorPubKey, TendermintVotePower);
impl From<&Validator> for ValidatorCandidate {
    fn from(v: &Validator) -> ValidatorCandidate {
        (v.node.consensus_pubkey.clone(), v.voting_power())
    }
}

// pub fn build_validator_update(
//     key: &TendermintValidatorPubKey,
//     power: TendermintVotePower,
// ) -> ValidatorUpdate {
//     let mut result = ValidatorUpdate::new();
//
//     let (keytype, key) = key.to_validator_update();
//     let mut pubkey = PubKey::new();
//     pubkey.set_field_type(keytype);
//     pubkey.set_data(key);
//
//     result.set_pub_key(pubkey);
//     result.set_power(power.into());
//     result
// }

#[derive(Debug, Clone, Default, Encode, Decode)]
pub struct ValidatorSet {
    /// Index by validator address
    by_validator_address: BTreeMap<TendermintValidatorAddress, StakedStateAddress>,
    /// Sorted by voting power
    by_power: BTreeMap<TendermintVotePower, Vec<StakedStateAddress>>,
}

impl ValidatorSet {
    fn new() -> ValidatorSet {
        ValidatorSet {
            by_validator_address: BTreeMap::new(),
            by_power: BTreeMap::new(),
        }
    }

    fn insert(
        &mut self,
        validator_addr: TendermintValidatorAddress,
        staking_addr: StakedStateAddress,
        power: TendermintVotePower,
    ) {
        assert!(
            self.by_validator_address
                .insert(validator_addr, staking_addr)
                .is_none(),
            "validator already exists"
        );
        self.by_power
            .entry(power)
            .and_modify(|xs| xs.push(staking_addr))
            .or_insert_with(|| vec![staking_addr]);
    }

    pub fn remove(&mut self, v: &StakedState) -> Option<()> {
        let addr = v.validator_address().unwrap();
        self.by_validator_address.remove(&addr).map(|_| {
            remove_item(
                self.by_power.get_mut(&v.voting_power()).unwrap(),
                &v.address,
            )
            .unwrap();
        })
    }

    pub fn get_by_staking(&self, staking: &StakedStateAddress) -> Option<&Validator> {
        self.by_staking_address
            .get(staking)
            .map(move |id| &self.validators[*id])
    }

    pub fn set_jailed(
        &mut self,
        staking: &StakedStateAddress,
        jailed: bool,
    ) -> Option<TendermintVotePower> {
        if let Some(id) = self.by_staking_address.get(staking) {
            let v = &mut self.validators[*id];
            if v.jailed != jailed {
                let old_power = v.voting_power();
                v.jailed = jailed;
                update_power_index(&mut self.by_power, *id, old_power, v.voting_power());
                Some(old_power)
            } else {
                Some(v.voting_power())
            }
        } else {
            None
        }
    }

    pub fn set_bonded(
        &mut self,
        staking: &StakedStateAddress,
        bonded: Coin,
    ) -> Option<TendermintVotePower> {
        if let Some(id) = self.by_staking_address.get(staking) {
            let v = &mut self.validators[*id];
            if bonded != v.bonded {
                let old_power = v.voting_power();
                v.bonded = bonded;
                update_power_index(&mut self.by_power, *id, old_power, v.voting_power());
                Some(old_power)
            } else {
                Some(v.voting_power())
            }
        } else {
            None
        }
    }

    pub fn sanity_check(&self) {
        assert_eq!(self.validators.len(), self.by_staking_address.len());
        assert_eq!(self.validators.len(), self.by_validator_address.len());
        assert_eq!(self.validators.len(), self.sorted_by_power().count());

        for (addr, i) in self.by_validator_address.iter() {
            assert_eq!(addr, &self.validators[*i].validator_address());
        }

        for (addr, i) in self.by_staking_address.iter() {
            assert_eq!(addr, &self.validators[*i].staking_address);
        }

        for (power, v) in self.sorted_by_power() {
            assert_eq!(power, v.voting_power());
        }
    }

    pub fn sorted_by_power(&self) -> impl Iterator<Item = (TendermintVotePower, &Validator)> {
        self.by_power
            .iter()
            .rev()
            .flat_map(move |(power, ids)| ids.iter().map(move |id| (*power, &self.validators[*id])))
    }

    pub fn iter_candidates(
        &self,
        min_stake: Coin,
        max_count: usize,
    ) -> impl Iterator<Item = &Validator> {
        let min_power = TendermintVotePower::from(min_stake);
        self.sorted_by_power()
            .filter_map(|(p, v)| if p >= min_power { Some(v) } else { None })
            .take(max_count)
    }

    /// Valid validator candidates
    pub fn candidates(&self, min_stake: Coin, max_count: usize) -> Vec<ValidatorCandidate> {
        self.iter_candidates(min_stake, max_count)
            .map(|v| v.into())
            .collect()
    }

    fn update_liveness(
        &mut self,
        addr: &TendermintValidatorAddress,
        height: BlockHeight,
        signed: bool,
    ) -> Option<()> {
        if let Some(id) = self.by_validator_address.get(addr) {
            let v = &mut self.validators[*id];
            v.liveness.update(height, signed);
            Some(())
        } else {
            None
        }
    }

    pub fn update_livenesses(
        &mut self,
        height: BlockHeight,
        commit_info: &[(TendermintValidatorAddress, bool)],
    ) {
        for (addr, signed) in commit_info {
            self.update_liveness(addr, height, *signed).unwrap();
        }
    }
}

pub fn diff_candidates(
    new: &[ValidatorCandidate],
    old: &[ValidatorCandidate],
) -> Vec<ValidatorCandidate> {
    let mut updates = new.to_owned();
    let new_map: HashMap<_, _> = new.iter().cloned().collect();

    for (key, _) in old.iter() {
        if new_map.get(key).is_none() {
            updates.push((key.clone(), TendermintVotePower::zero()))
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use chain_core::state::tendermint::TendermintValidatorPubKey;

    const BLOCK_SIGNING_WINDOW: u16 = 10;
    fn new_validator(pubkey: [u8; 32], staking: &str, bonded: u64) -> Validator {
        Validator::new(
            CouncilNode::new(TendermintValidatorPubKey::Ed25519(pubkey)),
            staking.parse().unwrap(),
            Coin::new(bonded).unwrap(),
            BLOCK_SIGNING_WINDOW,
        )
    }

    #[test]
    fn validator_set_sanity_check() {
        let mut set = ValidatorSet::new();
        let v1 = new_validator(
            [1u8; 32],
            "0x7c1691e7ff768c83da2a2a6e22484adefc746c8f",
            98_0000_0000,
        );
        set.insert(v1.clone());
        set.insert(new_validator(
            [2u8; 32],
            "0x1871fa49e9561b4257c133f300040a511957d310",
            99_0000_0000,
        ));
        set.insert(new_validator(
            [3u8; 32],
            "0x3c70ee44df1df96cf08f3906e446b9fe32a4c783",
            100_0000_0000,
        ));
        set.sanity_check();
        set.set_bonded(&v1.staking_address, Coin::new(101_0000_0000).unwrap())
            .unwrap();
        set.sanity_check();
        set.set_jailed(&v1.staking_address, true).unwrap();
        set.sanity_check();
    }
}
