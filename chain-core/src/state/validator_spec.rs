/// https://github.com/yihuang/chain-docs/blob/master/docs/getting-started/validator-spec.md
use std::cmp::Ordering;
use std::collections::{hash_map, BTreeMap, HashMap};

use slab::Slab;

// #configs
const MINIMAL_REQUIRED_STAKING: u64 = 1_0000_0000;
const MAX_VALIDATORS: u64 = 10;
const MAX_EVIDENCE_AGE: u64 = 10;
const SLASH_WAIT_PERIOD: u64 = 15;
const JAIL_DURATION: u64 = 18;
const REWARD_PERIOD: u64 = 10;

type StakingAddress = String;
type ValidatorAddress = String;
type SlashRate = u64;
type VotingPower = u64;
type Coin = u64;
type Timestamp = u64;

/// #table-schema
pub struct Validator {
    pub staking_address: StakingAddress,
    pub bonded_coins: Coin,
    pub validator_address: ValidatorAddress,
    pub council_node_info: String,
    pub inactive_time: Option<Timestamp>,
}

pub enum ValidatorUniqueViolation {
    StakingAddress,
    ValidatorAddress,
}

impl Validator {
    pub fn voting_power(&self) -> VotingPower {
        self.bonded_coins / 1_0000_0000_u64
    }

    pub fn is_active(&self) -> bool {
        self.inactive_time.is_none()
    }
}

/// order by bonded_coins desc, staking_address
#[derive(PartialEq, Eq)]
pub struct ValidatorSortKey(Coin, StakingAddress);

impl Ord for ValidatorSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.cmp(&other.0) {
            Ordering::Equal => self.1.cmp(&other.1),
            ordering => ordering,
        }
    }
}

impl PartialOrd for ValidatorSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
pub struct ValidatorSet {
    pub heap: Slab<Validator>,
    pub idx_staking_address: HashMap<StakingAddress, usize>,
    pub idx_sort_key: BTreeMap<ValidatorSortKey, usize>,
    pub idx_validator_address: HashMap<ValidatorAddress, usize>,
}

impl ValidatorSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Validator)> {
        self.heap.iter()
    }

    /// select * from validator order by bonded_coins desc, staking_address
    pub fn iter_sorted(&self) -> impl Iterator<Item = &Validator> {
        self.idx_sort_key
            .values()
            .copied()
            .map(move |oid| unsafe { self.heap.get_unchecked(oid) })
    }

    pub fn remove(&mut self, oid: usize) {
        let v = self.heap.remove(oid);
        // update indexes
        self.idx_staking_address.remove(&v.staking_address);
        self.idx_validator_address.remove(&v.validator_address);
        self.idx_sort_key
            .remove(&ValidatorSortKey(v.bonded_coins, v.staking_address));
    }

    /// ```sql
    /// select * from validator where staking_address=:staking_address
    /// ```
    pub fn get(&self, addr: &str) -> Option<&Validator> {
        self.idx_staking_address
            .get(addr)
            .map(move |oid| &self.heap[*oid])
    }

    /// ```sql
    /// select * from validator where staking_address=:staking_address
    /// ```
    pub fn get_mut(&mut self, addr: &str) -> Option<(usize, &mut Validator)> {
        let heap = &mut self.heap;
        self.idx_staking_address
            .get(addr)
            .map(move |oid| (*oid, &mut heap[*oid]))
    }

    /// ```sql
    /// select * from validator where validator_address=:validator_address
    /// ```
    pub fn get_by_validator_address(&self, validator_address: &str) -> Option<&Validator> {
        self.idx_validator_address
            .get(validator_address)
            .map(|oid| &self.heap[*oid])
    }

    /// ```sql
    /// update validator set bonded_coins=:value where staking_address=:staking_address
    /// ```
    pub fn set_bonded_coin(&mut self, staking_address: &str, value: Coin) {
        self.get_mut(staking_address).unwrap().1.bonded_coins = value;
    }

    /// ```sql
    /// insert into validator values (:staking_address, :bonded_coins, :validator_address, :council_node_info)
    /// returning *
    /// ```
    pub fn insert(
        &mut self,
        staking_address: StakingAddress,
        bonded_coins: Coin,
        validator_address: ValidatorAddress,
        council_node_info: String,
    ) -> Result<&Validator, ValidatorUniqueViolation> {
        // check unique contraints
        if self.idx_staking_address.contains_key(&staking_address) {
            return Err(ValidatorUniqueViolation::StakingAddress);
        }
        if self.idx_validator_address.contains_key(&validator_address) {
            return Err(ValidatorUniqueViolation::ValidatorAddress);
        }
        let oid = self.heap.insert(Validator {
            staking_address: staking_address.clone(),
            bonded_coins,
            validator_address: validator_address.clone(),
            council_node_info,
            inactive_time: None,
        });
        self.idx_staking_address
            .insert(staking_address.clone(), oid);
        self.idx_sort_key
            .insert(ValidatorSortKey(bonded_coins, staking_address), oid);
        self.idx_validator_address.insert(validator_address, oid);
        Ok(&self.heap[oid])
    }
}

/// #table-schema
pub struct Punishment {
    pub staking_address: StakingAddress,
    pub slash_rate: SlashRate,
    pub jail_time: Timestamp,
    pub slash_reason: String,
    pub slash_amount: Option<Coin>,
}

#[derive(Default)]
pub struct PunishmentSet {
    pub heap: Slab<Punishment>,
    pub idx_staking_address: HashMap<StakingAddress, usize>,
    // TODO implement the partial index on jail_time
}

pub enum PunishmentUniqueViolation {
    StakingAddress,
}

impl PunishmentSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut Punishment)> {
        self.heap.iter_mut()
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Punishment)> {
        self.heap.iter()
    }

    /// ```sql
    /// exists (select 1 from punishment where staking_address=:staking_address)
    /// ```
    pub fn contains(&self, staking_address: &str) -> bool {
        self.idx_staking_address.contains_key(staking_address)
    }

    /// ```sql
    /// select * from punishment where staking_address=:staking_address
    /// ```
    pub fn get(&self, staking_address: &str) -> Option<(usize, &Punishment)> {
        self.idx_staking_address
            .get(staking_address)
            .map(move |oid| (*oid, &self.heap[*oid]))
    }

    /// ```sql
    /// select * from punishment where staking_address=:staking_address
    /// ```
    pub fn get_mut(&mut self, validator_address: &str) -> Option<(usize, &mut Punishment)> {
        let heap = &mut self.heap;
        self.idx_staking_address
            .get(validator_address)
            .map(move |oid| (*oid, &mut heap[*oid]))
    }

    pub fn remove(&mut self, oid: usize) -> Punishment {
        let v = self.heap.remove(oid);
        self.idx_staking_address.remove(&v.staking_address);
        v
    }

    /// ```sql
    /// insert into punishment values (:staking_address, :slash_rate, :block_time, :slash_reason)
    /// returning *
    /// ```
    pub fn insert(
        &mut self,
        staking_address: StakingAddress,
        slash_rate: SlashRate,
        block_time: Timestamp,
        slash_reason: String,
    ) -> Result<&Punishment, PunishmentUniqueViolation> {
        match self.idx_staking_address.entry(staking_address.clone()) {
            hash_map::Entry::Vacant(entry) => {
                let oid = self.heap.insert(Punishment {
                    staking_address,
                    slash_rate,
                    jail_time: block_time,
                    slash_reason,
                    slash_amount: None,
                });
                entry.insert(oid);
                Ok(&self.heap[oid])
            }
            _ => Err(PunishmentUniqueViolation::StakingAddress),
        }
    }
}

#[derive(Default)]
pub struct ValidatorState {
    validator: ValidatorSet,
    validator_snapshot: HashMap<StakingAddress, VotingPower>,
    reward_stat: HashMap<StakingAddress, u64>,
    punishment: PunishmentSet,
    last_reward_distributio_time: Timestamp,
}

/// #generate-validator-updates
fn diff_validators(
    old: &HashMap<ValidatorAddress, VotingPower>,
    new: &HashMap<ValidatorAddress, VotingPower>,
) -> Vec<(ValidatorAddress, VotingPower)> {
    // updates + removes
    new.iter()
        .filter_map(|(addr, power)| {
            if old.get::<str>(addr) != Some(power) {
                Some((addr.clone(), *power))
            } else {
                None
            }
        })
        .chain(old.iter().filter_map(|(addr, _)| {
            if !new.contains_key(addr) {
                Some((addr.clone(), 0))
            } else {
                None
            }
        }))
        .collect::<Vec<_>>()
}

impl ValidatorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// #set-inactive_time-for-newly-inactive-validators
    /// Complexity: O(N * log(N))
    pub fn set_inactive_time(&mut self, block_time: Timestamp) {
        // get validator oid set
        for (_, v) in self.validator.heap.iter_mut() {
            if v.inactive_time.is_none()
                && (v.bonded_coins < MINIMAL_REQUIRED_STAKING
                    || self.punishment.contains(&v.staking_address))
            {
                v.inactive_time = Some(block_time);
            }
        }
    }

    /// #generate-validator-updates
    pub fn gen_validator_update(&mut self) -> Vec<(StakingAddress, VotingPower)> {
        let new = self
            .validator
            .iter_sorted()
            .filter(|v| v.is_active())
            .map(|v| (v.validator_address.clone(), v.voting_power()))
            .take(MAX_VALIDATORS as usize)
            .collect::<HashMap<_, _>>();
        let updates = diff_validators(&self.validator_snapshot, &new);
        self.validator_snapshot = new;
        updates
    }

    /// #cleanup
    pub fn cleanup(&mut self, block_time: Timestamp) {
        let oids = self
            .validator
            .iter()
            .filter_map(|(oid, v)| {
                if let Some(inactive_time) = v.inactive_time {
                    if block_time > MAX_EVIDENCE_AGE + inactive_time
                        && !self.reward_stat.contains_key(&v.staking_address)
                        && !self.punishment.contains(&v.staking_address)
                    {
                        return Some(oid);
                    }
                }
                None
            })
            .collect::<Vec<_>>();
        for oid in oids.into_iter() {
            self.validator.remove(oid);
        }
    }

    /// #join-node
    pub fn join_node(
        &mut self,
        staking_address: StakingAddress,
        bonded_coins: Coin,
        validator_address: ValidatorAddress,
        council_node_info: String,
    ) -> bool {
        match self.validator.insert(
            staking_address.clone(),
            bonded_coins,
            validator_address.clone(),
            council_node_info.clone(),
        ) {
            Ok(_) => true,
            Err(ValidatorUniqueViolation::StakingAddress) => {
                // re-join
                let (oid, v) = self.validator.get_mut(&staking_address).unwrap();
                if v.bonded_coins >= MINIMAL_REQUIRED_STAKING
                    && !self.punishment.contains(&staking_address)
                {
                    assert_eq!(v.bonded_coins, bonded_coins);
                    v.council_node_info = council_node_info;
                    v.validator_address = validator_address.clone();
                    self.validator
                        .idx_validator_address
                        .insert(validator_address, oid);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// #unjail
    pub fn unjail(&mut self, staking_address: &str, block_time: Timestamp) -> bool {
        if let Some((oid, v)) = self.punishment.get(staking_address) {
            if block_time >= v.jail_time + JAIL_DURATION {
                self.punishment.remove(oid);
                return true;
            }
        }
        false
    }

    /// #jail-byzantine-or-non-liveness
    pub fn jail(
        &mut self,
        block_time: Timestamp,
        punishments: Vec<(StakingAddress, SlashRate, String)>,
    ) {
        for (staking_address, slash_rate, slash_reason) in punishments.into_iter() {
            match self.punishment.insert(
                staking_address.clone(),
                slash_rate,
                block_time,
                slash_reason.clone(),
            ) {
                Ok(_) => {}
                Err(PunishmentUniqueViolation::StakingAddress) => {
                    // update punishment
                    let (_, v) = self.punishment.get_mut(&staking_address).unwrap();
                    if v.slash_amount.is_none() && slash_rate > v.slash_rate {
                        v.slash_rate = slash_rate;
                        v.slash_reason = slash_reason;
                    }
                }
            }
        }
    }

    /// #slash
    pub fn slash(&mut self, block_time: Timestamp) {
        for (_, v) in self.punishment.iter_mut() {
            if v.slash_amount.is_none() && block_time >= v.jail_time + SLASH_WAIT_PERIOD {
                // TODO execute slash
                v.slash_amount = Some(1);
            }
        }
    }

    /// #reward-distribution
    pub fn distribute_reward(&mut self, block_time: Timestamp, block_proposer: &str) {
        if let Some(v) = self.validator.get_by_validator_address(block_proposer) {
            self.reward_stat
                .entry(v.staking_address.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }

        if block_time.saturating_sub(self.last_reward_distributio_time) >= REWARD_PERIOD {
            self.last_reward_distributio_time = block_time;

            let total_reward = 1_0000_0000_u64; // TODO
            let blocks: u64 = self.reward_stat.iter().map(|(_, c)| c).sum();
            let share = total_reward / blocks;
            for (addr, c) in self.reward_stat.iter() {
                // TODO
                println!("reward: {} {}", addr, share * c);
            }

            self.reward_stat.clear();
        }
    }

    /// #end_block
    pub fn end_block(&mut self, block_time: Timestamp) -> Vec<(ValidatorAddress, VotingPower)> {
        self.set_inactive_time(block_time);
        let updates = self.gen_validator_update();
        self.cleanup(block_time);
        updates
    }

    /// #begin_block
    pub fn begin_block(
        &mut self,
        block_time: Timestamp,
        block_proposer: &str,
        punishments: Vec<(StakingAddress, SlashRate, String)>,
    ) {
        self.jail(block_time, punishments);
        self.slash(block_time);
        self.distribute_reward(block_time, block_proposer);
    }
}
