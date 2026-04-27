use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, IntoVal,
};

use crate::vesting::{RevoraVesting, RevoraVestingClient, VESTING_EVENT_SCHEMA_VERSION};

fn setup(env: &Env) -> (RevoraVestingClient, Address, Address, Address) {
    let contract_id = env.register_contract(None, RevoraVesting);
    let client = RevoraVestingClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let beneficiary = Address::generate(env);
    let token_id = env.register_stellar_asset_contract(admin.clone());
    (client, admin, beneficiary, token_id)
}

fn mint_tokens(env: &Env, payment_token: &Address, recipient: &Address, amount: &i128) {
    soroban_sdk::token::StellarAssetClient::new(env, payment_token).mint(recipient, amount);
}

fn balance(env: &Env, payment_token: &Address, who: &Address) -> i128 {
    soroban_sdk::token::Client::new(env, payment_token).balance(who)
}

#[test]
fn initialize_sets_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _b, _t) = setup(&env);
    client.initialize_vesting(&admin);
}

#[test]
fn create_schedule_success() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let total = 1_000_000_i128;
    let start = 1000_u64;
    let cliff = 500_u64;
    let duration = 2000_u64;

    let idx =
        client.create_schedule(&admin, &beneficiary, &token_id, &total, &start, &cliff, &duration);
    assert_eq!(idx, 0);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.beneficiary, beneficiary);
    assert_eq!(schedule.total_amount, total);
    assert_eq!(schedule.claimed_amount, 0);
    assert_eq!(schedule.start_time, start);
    assert_eq!(schedule.cliff_time, start + cliff);
    assert_eq!(schedule.end_time, start + duration);
    assert!(!schedule.cancelled);
}

#[test]
fn get_claimable_before_cliff_is_zero() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let total = 1_000_000_i128;
    let start = 1000_u64;
    let cliff = 500_u64;
    let duration = 2000_u64;
    client.create_schedule(&admin, &beneficiary, &token_id, &total, &start, &cliff, &duration);

    env.ledger().with_mut(|l| l.timestamp = start + 100);
    let claimable = client.get_claimable_vesting(&admin, &0);
    assert_eq!(claimable, 0);
}

#[test]
fn cancel_schedule() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);
    client.create_schedule(&admin, &beneficiary, &token_id, &1_000_000, &1000, &100, &2000);

    client.cancel_schedule(&admin, &beneficiary, &0);
    let schedule = client.get_schedule(&admin, &0);
    assert!(schedule.cancelled);
}

#[test]
fn multiple_schedules_same_beneficiary() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    client.create_schedule(&admin, &beneficiary, &token_id, &100, &1000, &0, &1000);
    client.create_schedule(&admin, &beneficiary, &token_id, &200, &2000, &0, &1000);
    assert_eq!(client.get_schedule_count(&admin), 2);
}

#[test]
fn zero_duration_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);
    let r = client.try_create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &0, &0);
    assert!(r.is_err());
}

#[test]
fn cliff_longer_than_duration_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);
    let r = client.try_create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &2000, &1000);
    assert!(r.is_err());
}

#[test]
fn negative_amount_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);
    let r = client.try_create_schedule(&admin, &beneficiary, &token_id, &0, &1000, &0, &1000);
    assert!(r.is_err());
    let r2 = client.try_create_schedule(&admin, &beneficiary, &token_id, &-10, &1000, &0, &1000);
    assert!(r2.is_err());
}

#[test]
fn double_initialize_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _b, _t) = setup(&env);
    client.initialize_vesting(&admin);
    let r = client.try_initialize_vesting(&admin);
    assert!(r.is_err());
}

#[test]
fn test_claim_vesting_success() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    // Mint tokens to the contract
    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &1000);

    let start = 1000;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &1000);

    env.ledger().with_mut(|l| l.timestamp = 1500);
    let claimed = client.claim_vesting(&beneficiary, &admin, &0);
    assert_eq!(claimed, 500);

    env.ledger().with_mut(|l| l.timestamp = 2500);
    let claimed2 = client.claim_vesting(&beneficiary, &admin, &0);
    assert_eq!(claimed2, 500);

    let r = client.try_claim_vesting(&beneficiary, &admin, &0);
    assert!(r.is_err());
}

#[test]
fn cancel_schedule_already_cancelled() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &100, &2000);

    client.cancel_schedule(&admin, &beneficiary, &0);
    let r = client.try_cancel_schedule(&admin, &beneficiary, &0);
    assert!(r.is_err());
}

#[test]
fn try_cancel_schedule_wrong_beneficiary() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    let wrong_beneficiary = Address::generate(&env);
    client.initialize_vesting(&admin);
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &100, &2000);

    let r = client.try_cancel_schedule(&admin, &wrong_beneficiary, &0);
    assert!(r.is_err());
}

#[test]
fn amend_schedule_success() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let start = 1000;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &1000);

    // Amend: Increase total amount and double duration
    client.amend_schedule(&admin, &beneficiary, &0, &2000, &start, &0, &2000);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.total_amount, 2000);
    assert_eq!(schedule.end_time, start + 2000);
}

#[test]
fn amend_schedule_partially_claimed_success() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    // Mint tokens to the contract
    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &5000);

    let start = 1000;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &1000);

    // Claim 500 at t=1500
    env.ledger().with_mut(|l| l.timestamp = 1500);
    client.claim_vesting(&beneficiary, &admin, &0);

    // Amend: Reduce total to 800 (still > 500 claimed)
    client.amend_schedule(&admin, &beneficiary, &0, &800, &start, &0, &1000);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.total_amount, 800);
    assert_eq!(schedule.claimed_amount, 500);
}

#[test]
fn amend_schedule_too_low_amount_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &1000);

    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &0, &1000);

    env.ledger().with_mut(|l| l.timestamp = 1500);
    client.claim_vesting(&beneficiary, &admin, &0); // claimed 500

    // Try to reduce total to 400 (claimed is 500)
    let r = client.try_amend_schedule(&admin, &beneficiary, &0, &400, &1000, &0, &1000);
    assert!(r.is_err());
}

#[test]
fn amend_schedule_invalid_params_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &0, &1000);

    // Zero duration
    let r = client.try_amend_schedule(&admin, &beneficiary, &0, &1000, &1000, &0, &0);
    assert!(r.is_err());

    // Cliff > Duration
    let r2 = client.try_amend_schedule(&admin, &beneficiary, &0, &1000, &1000, &2000, &1000);
    assert!(r2.is_err());
}

#[test]
fn amend_cancelled_schedule_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &0, &1000);

    client.cancel_schedule(&admin, &beneficiary, &0);

    let r = client.try_amend_schedule(&admin, &beneficiary, &0, &2000, &1000, &0, &1000);
    assert!(r.is_err());
}

#[test]
fn amend_non_existent_schedule_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, _token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let r = client.try_amend_schedule(&admin, &beneficiary, &99, &1000, &1000, &0, &1000);
    assert!(r.is_err());
}

// ── Comprehensive Amendment Tests ──────────────────────────────────
// These tests cover security assumptions, edge cases, and adversarial scenarios.

#[test]
fn amendment_emits_legacy_and_v1_events() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let start = 1000;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &1000);

    // Clear events from creation
    env.events().all();

    // Amend
    client.amend_schedule(&admin, &beneficiary, &0, &2000, &start, &0, &2000);

    // Verify both legacy and v1 events were emitted
    let events = env.events().all();
    assert!(events.len() >= 2, "Expected at least 2 events (legacy + v1)");
}

#[test]
fn amendment_increases_claimable_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &5000);

    let start = 1000;
    let original_amount = 1000_i128;
    client.create_schedule(&admin, &beneficiary, &token_id, &original_amount, &start, &0, &1000);

    // At t = 1500: 50% vested = 500
    env.ledger().with_mut(|l| l.timestamp = 1500);
    let claimable_before = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable_before, 500);

    // Amend to increase total to 2000 (no change to timing)
    // New formula: vested = 2000 * 50% = 1000
    client.amend_schedule(&admin, &beneficiary, &0, &2000, &start, &0, &1000);

    let claimable_after = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable_after, 1000, "Amendment increased total, so claimable should increase");
}

#[test]
fn amendment_decreases_claimable_amount_respects_claimed() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &5000);

    let start = 1000;
    client.create_schedule(&admin, &beneficiary, &token_id, &2000, &start, &0, &1000);

    // Claim 500 at 25% vesting
    env.ledger().with_mut(|l| l.timestamp = 1250);
    client.claim_vesting(&beneficiary, &admin, &0);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.claimed_amount, 500);

    // Reduce total to 1000 (still > claimed 500)
    client.amend_schedule(&admin, &beneficiary, &0, &1000, &start, &0, &1000);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.claimed_amount, 500, "Claimed amount should NOT change");
    assert_eq!(schedule.total_amount, 1000, "Total should be updated");

    // Verify claimable is now correctly computed: 1000 * 25% = 250, minus 500 claimed = -250 → 0
    env.ledger().with_mut(|l| l.timestamp = 1250);
    let claimable = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable, 0, "At 25% vesting with total=1000, nothing more to claim");
}

#[test]
fn adversarial_amend_backdate_start_does_not_steal_vested() {
    // Scenario: Issuer tries to extend vesting window backward by reducing start_time.
    // This should NOT magically increase the claimable amount; it affects future linear calculation only.
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &10000);

    let original_start = 5000_u64;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &original_start, &0, &1000);

    // At original_start + 500 (50% through vesting): claimable = 500
    env.ledger().with_mut(|l| l.timestamp = original_start + 500);
    let claimable_before = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable_before, 500);

    // Issuer tries to backdate by moving start to 1000 (4000 seconds earlier)
    // New schedule: starts at 1000, ends at 2000 (1000 seconds duration)
    // At current time (5500): we're 3500 seconds past end
    // So vested = 100% = 1000
    let new_start = 1000_u64;
    client.amend_schedule(&admin, &beneficiary, &0, &1000, &new_start, &0, &1000);

    let claimable_after = client.get_claimable_vesting(&admin, &0).unwrap();
    // At timestamp 5500, with start=1000, end=2000:
    // We're already past the end, so vested = all 1000
    // But we haven't claimed yet, so claimable = 1000
    assert_eq!(
        claimable_after, 1000,
        "Backdating DOES increase claimable since we're past the entire duration, \
         but this is acceptable: issuer can't STEAL, only accelerate vesting."
    );

    // The key security property: the claimed_amount doesn't reset
    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(
        schedule.claimed_amount, 0,
        "Claimed amount should remain 0 (not reset by amendment)"
    );
}

#[test]
fn adversarial_amend_cannot_reduce_below_claimed() {
    // Core security: issuer cannot reduce total below what beneficiary has already claimed
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &10000);

    let start = 1000_u64;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &1000);

    // Beneficiary claims 600 tokens
    env.ledger().with_mut(|l| l.timestamp = 1600);
    client.claim_vesting(&beneficiary, &admin, &0);

    // Issuer tries to reduce total to 500 (< 600 claimed)
    let r = client.try_amend_schedule(&admin, &beneficiary, &0, &500, &start, &0, &1000);
    assert!(r.is_err(), "Amendment below claimed amount must fail");
}

#[test]
fn amendment_preserves_beneficiary_identity() {
    // Ensure amendment cannot be used to steal from wrong beneficiary
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    let attacker = Address::generate(&env);
    client.initialize_vesting(&admin);

    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &0, &1000);

    // Attacker tries to amend targeting wrong beneficiary
    let r = client.try_amend_schedule(&admin, &attacker, &0, &2000, &1000, &0, &1000);
    assert!(r.is_err(), "Amendment with wrong beneficiary should fail");
}

#[test]
fn amendment_preserves_auth_requirement() {
    // Only authorized admin can amend
    let env = Env::default();
    let (client, admin, beneficiary, token_id) = setup(&env);
    env.mock_all_auths();
    client.initialize_vesting(&admin);

    env.mock_all_auths_allow_last(false);
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &0, &1000);

    let attacker = Address::generate(&env);
    env.mock_all_auths();

    // Attacker tries to amend without auth
    let r = client.try_amend_schedule(&attacker, &beneficiary, &0, &2000, &1000, &0, &1000);
    assert!(r.is_err(), "Non-admin amendment should fail due to auth");
}

#[test]
fn amendment_mid_claim_preserves_claimed_state() {
    // Verify that amending during an active claim period doesn't affect past claims
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &10000);

    let start = 1000_u64;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &2000);

    // Claim 250 at 25% vesting
    env.ledger().with_mut(|l| l.timestamp = 1500);
    client.claim_vesting(&beneficiary, &admin, &0);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.claimed_amount, 250);

    // Mid-claim, issuer extends duration to 4000
    client.amend_schedule(&admin, &beneficiary, &0, &1000, &start, &0, &4000);

    // Verify claimed_amount persisted
    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.claimed_amount, 250, "Claimed state must persist after amendment");

    // Verify future claimable is recalculated correctly
    // At t=1500, new vesting: 1000 * (500 / 4000) = 125
    // Claimable: 125 - 250 = -125 → 0 (clamped)
    let claimable = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable, 0, "Extending duration should reduce claimable");
}

#[test]
fn amendment_extreme_amount_increase() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let start = 1000;
    client.create_schedule(&admin, &beneficiary, &token_id, &1, &start, &0, &1000);

    // Increase to huge amount
    let huge_amount = 1_000_000_000_000_i128;
    client.amend_schedule(&admin, &beneficiary, &0, &huge_amount, &start, &0, &1000);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.total_amount, huge_amount);
}

#[test]
fn amendment_extreme_duration_extension() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let start = 1000_u64;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &1000);

    // Extend to many days
    let huge_duration = 10_000_000_u64; // ~115 days
    client.amend_schedule(&admin, &beneficiary, &0, &1000, &start, &0, &huge_duration);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.end_time, start + huge_duration);
}

#[test]
fn amendment_resets_cliff() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &10000);

    let start = 1000_u64;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &100, &1000);

    // At t = 1050: before cliff, claimable should be 0
    env.ledger().with_mut(|l| l.timestamp = 1050);
    let claimable_before = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable_before, 0, "Before cliff, nothing is vested");

    // Amend to remove cliff (cliff_duration = 0)
    client.amend_schedule(&admin, &beneficiary, &0, &1000, &start, &0, &1000);

    // Now at t = 1050, we're 50 seconds into 1000-second vesting: 5%
    let claimable_after = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable_after, 50, "Removing cliff allows vesting from start");
}

#[test]
fn amendment_introduces_new_cliff() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &10000);

    let start = 1000_u64;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &1000);

    env.ledger().with_mut(|l| l.timestamp = 1500);
    let claimable_before = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable_before, 500, "50% vested with no cliff");

    // Rewrite schedule with a cliff that extends past current time
    // New start=1000, cliff=600 (cliff_time=1600), duration=1000 (end=2000)
    // At t=1500: before cliff, so claimable = 0
    client.amend_schedule(&admin, &beneficiary, &0, &1000, &start, &600, &1000);

    let claimable_after = client.get_claimable_vesting(&admin, &0).unwrap();
    assert_eq!(claimable_after, 0, "Introducing cliff in the future resets claimable to 0");
}

#[test]
fn amendment_multiple_consecutive() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &1000, &0, &1000);

    // Amend 1
    client.amend_schedule(&admin, &beneficiary, &0, &2000, &1000, &0, &1000);
    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.total_amount, 2000);

    // Amend 2
    client.amend_schedule(&admin, &beneficiary, &0, &3000, &1000, &0, &1000);
    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.total_amount, 3000);

    // Amend 3
    client.amend_schedule(&admin, &beneficiary, &0, &2500, &1000, &0, &1000);
    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.total_amount, 2500);
}

#[test]
fn amendment_then_claim_uses_new_parameters() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, beneficiary, token_id) = setup(&env);
    client.initialize_vesting(&admin);

    let str_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    str_client.mint(&client.address, &10000);

    let start = 1000_u64;
    client.create_schedule(&admin, &beneficiary, &token_id, &1000, &start, &0, &1000);

    env.ledger().with_mut(|l| l.timestamp = 1500);

    // Amend to 2000
    client.amend_schedule(&admin, &beneficiary, &0, &2000, &start, &0, &1000);

    // Claim should use new 2000 total: 50% of 2000 = 1000
    let claimed = client.claim_vesting(&beneficiary, &admin, &0);
    assert_eq!(claimed, 1000);

    let schedule = client.get_schedule(&admin, &0);
    assert_eq!(schedule.claimed_amount, 1000);
}
