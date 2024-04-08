use crate::{mock::*, Event, PayerKey};
use frame_support::assert_ok;

#[test]
fn it_works_for_default_value() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		// Dispatch a signed extrinsic.
		assert_ok!(Spontra::set_payer_key(RuntimeOrigin::root(), 1));
		// Read pallet storage and assert an expected result.
		assert_eq!(PayerKey::<Test>::get(), Some(1));
		// Assert that the correct event was deposited
		System::assert_last_event(Event::PayerKeyUpdated { old: None, new: 1 }.into());
	});
}
