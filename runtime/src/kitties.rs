use support::{
	decl_module, decl_storage, decl_event, ensure, StorageValue, StorageMap,
	Parameter, traits::Currency
};
use runtime_primitives::traits::{SimpleArithmetic, Bounded, One, Member};
use parity_codec::{Encode, Decode};
use runtime_io::blake2_128;
use system::ensure_signed;
use rstd::result;
use crate::linked_item::{LinkedList, LinkedItem};

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type KittyIndex: Parameter + Member + SimpleArithmetic + Bounded + Default + Copy;
	type Currency: Currency<Self::AccountId>;
}

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

#[derive(Encode, Decode)]
pub struct Kitty(pub [u8; 16]);

type KittyLinkedItem<T> = LinkedItem<<T as Trait>::KittyIndex>;
type OwnedKittiesList<T> = LinkedList<OwnedKitties<T>, <T as system::Trait>::AccountId, <T as Trait>::KittyIndex>;

decl_storage! {
	trait Store for Module<T: Trait> as Kitties {
		/// Stores all the kitties, key is the kitty id / index
		pub Kitties get(kitty): map T::KittyIndex => Option<Kitty>;
		/// Stores the total number of kitties. i.e. the next kitty index
		pub KittiesCount get(kitties_count): T::KittyIndex;

		/// Get kitty ownership. Stored in a linked map.
		pub OwnedKitties get(owned_kitties): map (T::AccountId, Option<T::KittyIndex>) => Option<KittyLinkedItem<T>>;

		/// Get kitty owner
		pub KittyOwners get(kitty_owner): map T::KittyIndex => Option<T::AccountId>;

		/// Get kitty price. None means not for sale.
		pub KittyPrices get(kitty_price): map T::KittyIndex => Option<BalanceOf<T>>
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::KittyIndex,
		Balance = BalanceOf<T>,
	{
		/// A kitty is created. (owner, kitty_id)
		Created(AccountId, KittyIndex),
		/// A kitty is transferred. (from, to, kitty_id)
		Transferred(AccountId, AccountId, KittyIndex),
		/// A kitty is available for sale. (owner, kitty_id, price)
		Ask(AccountId, KittyIndex, Option<Balance>),
		/// A kitty is sold. (from, to, kitty_id, price)
		Sold(AccountId, AccountId, KittyIndex, Balance),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		/// Create a new kitty
		pub fn create(origin) {
			let sender = ensure_signed(origin)?;
			let kitty_id = Self::next_kitty_id()?;

			// Generate a random 128bit value
			let dna = Self::random_value(&sender);

			// Create and store kitty
			let kitty = Kitty(dna);
			Self::insert_kitty(&sender, kitty_id, kitty);

			Self::deposit_event(RawEvent::Created(sender, kitty_id));
		}

		/// Breed kitties
		pub fn breed(origin, kitty_id_1: T::KittyIndex, kitty_id_2: T::KittyIndex) {
			let sender = ensure_signed(origin)?;

			let new_kitty_id = Self::do_breed(&sender, kitty_id_1, kitty_id_2)?;

			Self::deposit_event(RawEvent::Created(sender, new_kitty_id));
		}

		/// Transfer a kitty to new owner
		pub fn transfer(origin, to: T::AccountId, kitty_id: T::KittyIndex) {
			let sender = ensure_signed(origin)?;

			ensure!(<OwnedKitties<T>>::exists(&(sender.clone(), Some(kitty_id))), "Only owner can transfer kitty");
			
			Self::do_transfer(&sender, &to, kitty_id);

			Self::deposit_event(RawEvent::Transferred(sender, to, kitty_id));
		}

		/// Set a price for a kitty for sale
		/// None to delist the kitty
		pub fn ask(origin, kitty_id: T::KittyIndex, price: Option<BalanceOf<T>>) {
			let sender = ensure_signed(origin)?;

			ensure!(<OwnedKitties<T>>::exists(&(sender.clone(), Some(kitty_id))), "Only owner can set price for kitty");

			if let Some(ref price) = price {
				<KittyPrices<T>>::insert(kitty_id, price);
			} else {
				<KittyPrices<T>>::remove(kitty_id);
			}
			
			Self::deposit_event(RawEvent::Ask(sender, kitty_id, price));
		}

		/// Buy a kitty with max price willing to pay
		pub fn buy(origin, kitty_id: T::KittyIndex, price: BalanceOf<T>) {
			let sender = ensure_signed(origin)?;

			let owner = Self::kitty_owner(kitty_id);
			ensure!(owner.is_some(), "Kitty does not exist");
			let owner = owner.unwrap();

			let kitty_price = Self::kitty_price(kitty_id);
			ensure!(kitty_price.is_some(), "Kitty not for sale");

			let kitty_price = kitty_price.unwrap();
			ensure!(price >= kitty_price, "Price is too low");

			T::Currency::transfer(&sender, &owner, kitty_price)?;

			<KittyPrices<T>>::remove(kitty_id);

			Self::do_transfer(&owner, &sender, kitty_id);

			Self::deposit_event(RawEvent::Sold(owner, sender, kitty_id, kitty_price));
		}
	}
}

fn combine_dna(dna1: u8, dna2: u8, selector: u8) -> u8 {
	((selector & dna1) | (!selector & dna2))
}

impl<T: Trait> Module<T> {
	fn random_value(sender: &T::AccountId) -> [u8; 16] {
		let payload = (<system::Module<T>>::random_seed(), sender, <system::Module<T>>::extrinsic_index(), <system::Module<T>>::block_number());
		payload.using_encoded(blake2_128)
	}

	fn next_kitty_id() -> result::Result<T::KittyIndex, &'static str> {
		let kitty_id = Self::kitties_count();
		if kitty_id == <T::KittyIndex as Bounded>::max_value() {
			return Err("Kitties count overflow");
		}
		Ok(kitty_id)
	}

	fn insert_owned_kitty(owner: &T::AccountId, kitty_id: T::KittyIndex) {
		<OwnedKittiesList<T>>::append(owner, kitty_id);
	}

	fn insert_kitty(owner: &T::AccountId, kitty_id: T::KittyIndex, kitty: Kitty) {
		// Create and store kitty
		<Kitties<T>>::insert(kitty_id, kitty);
		<KittiesCount<T>>::put(kitty_id + One::one());
		<KittyOwners<T>>::insert(kitty_id, owner.clone());

		Self::insert_owned_kitty(owner, kitty_id);
	}

	fn do_breed(sender: &T::AccountId, kitty_id_1: T::KittyIndex, kitty_id_2: T::KittyIndex) -> result::Result<T::KittyIndex, &'static str> {
		let kitty1 = Self::kitty(kitty_id_1);
		let kitty2 = Self::kitty(kitty_id_2);

		ensure!(kitty1.is_some(), "Invalid kitty_id_1");
		ensure!(kitty2.is_some(), "Invalid kitty_id_2");
		ensure!(kitty_id_1 != kitty_id_2, "Needs different parent");
		ensure!(Self::kitty_owner(&kitty_id_1).map(|owner| owner == *sender).unwrap_or(false), "Not owner of kitty1");
		ensure!(Self::kitty_owner(&kitty_id_2).map(|owner| owner == *sender).unwrap_or(false), "Not owner of kitty2");

		let kitty_id = Self::next_kitty_id()?;

		let kitty1_dna = kitty1.unwrap().0;
		let kitty2_dna = kitty2.unwrap().0;

		// Generate a random 128bit value
		let selector = Self::random_value(&sender);
		let mut new_dna = [0u8; 16];

		// Combine parents and selector to create new kitty
		for i in 0..kitty1_dna.len() {
			new_dna[i] = combine_dna(kitty1_dna[i], kitty2_dna[i], selector[i]);
		}

		Self::insert_kitty(sender, kitty_id, Kitty(new_dna));

		Ok(kitty_id)
	}

	fn do_transfer(from: &T::AccountId, to: &T::AccountId, kitty_id: T::KittyIndex)  {
		<OwnedKittiesList<T>>::remove(&from, kitty_id);
		<OwnedKittiesList<T>>::append(&to, kitty_id);
		<KittyOwners<T>>::insert(kitty_id, to);
	}
}

/// Tests for Kitties module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{impl_outer_origin, assert_ok, assert_noop};
	use runtime_primitives::{
		BuildStorage,
		traits::{BlakeTwo256, IdentityLookup},
		testing::{Digest, DigestItem, Header}
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	// For testing the module, we construct most of a mock runtime. This means
	// first constructing a configuration type (`Test`) which `impl`s each of the
	// configuration traits of modules we want to use.
	#[derive(Clone, Eq, PartialEq, Debug)]
	pub struct Test;
	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Digest = Digest;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type Log = DigestItem;
	}
	impl balances::Trait for Test {
		type Balance = u32;
		type OnFreeBalanceZero = ();
		type OnNewAccount = ();
		type Event = ();

		type TransactionPayment = ();
		type DustRemoval = ();
		type TransferPayment = ();
	}
	impl Trait for Test {
		type KittyIndex = u32;
		type Currency = balances::Module<Test>;
		type Event = ();
	}
	type Balances = balances::Module<Test>;
	type KittyModule = Module<Test>;
	type OwnedKittiesTest = OwnedKitties<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		let mut t = system::GenesisConfig::<Test>::default().build_storage().unwrap().0;
		t.extend(balances::GenesisConfig::<Test> {
			transaction_base_fee: 0,
			transaction_byte_fee: 0,
			balances: vec![(1, 10), (2, 20), (3, 30), (4, 40), (5, 50), (6, 60)],
			existential_deposit: 0,
			transfer_fee: 0,
			creation_fee: 0,
			vesting: vec![],
		}.build_storage().unwrap().0);
		t.into()
	}

	#[test]
	fn owned_kitties_can_append_values() {
		with_externalities(&mut new_test_ext(), || {
			OwnedKittiesList::<Test>::append(&0, 1);

			assert_eq!(OwnedKittiesTest::get(&(0, None)), Some(KittyLinkedItem::<Test> {
				prev: Some(1),
				next: Some(1),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(1))), Some(KittyLinkedItem::<Test> {
				prev: None,
				next: None,
			}));

			OwnedKittiesList::<Test>::append(&0, 2);

			assert_eq!(OwnedKittiesTest::get(&(0, None)), Some(KittyLinkedItem::<Test> {
				prev: Some(2),
				next: Some(1),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(1))), Some(KittyLinkedItem::<Test> {
				prev: None,
				next: Some(2),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(2))), Some(KittyLinkedItem::<Test> {
				prev: Some(1),
				next: None,
			}));

			OwnedKittiesList::<Test>::append(&0, 3);

			assert_eq!(OwnedKittiesTest::get(&(0, None)), Some(KittyLinkedItem::<Test> {
				prev: Some(3),
				next: Some(1),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(1))), Some(KittyLinkedItem::<Test> {
				prev: None,
				next: Some(2),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(2))), Some(KittyLinkedItem::<Test> {
				prev: Some(1),
				next: Some(3),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(3))), Some(KittyLinkedItem::<Test> {
				prev: Some(2),
				next: None,
			}));
		});
	}

	#[test]
	fn owned_kitties_can_remove_values() {
		with_externalities(&mut new_test_ext(), || {
			OwnedKittiesList::<Test>::append(&0, 1);
			OwnedKittiesList::<Test>::append(&0, 2);
			OwnedKittiesList::<Test>::append(&0, 3);

			OwnedKittiesList::<Test>::remove(&0, 2);

			assert_eq!(OwnedKittiesTest::get(&(0, None)), Some(KittyLinkedItem::<Test> {
				prev: Some(3),
				next: Some(1),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(1))), Some(KittyLinkedItem::<Test> {
				prev: None,
				next: Some(3),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(2))), None);

			assert_eq!(OwnedKittiesTest::get(&(0, Some(3))), Some(KittyLinkedItem::<Test> {
				prev: Some(1),
				next: None,
			}));

			OwnedKittiesList::<Test>::remove(&0, 1);

			assert_eq!(OwnedKittiesTest::get(&(0, None)), Some(KittyLinkedItem::<Test> {
				prev: Some(3),
				next: Some(3),
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(1))), None);

			assert_eq!(OwnedKittiesTest::get(&(0, Some(2))), None);

			assert_eq!(OwnedKittiesTest::get(&(0, Some(3))), Some(KittyLinkedItem::<Test> {
				prev: None,
				next: None,
			}));

			OwnedKittiesList::<Test>::remove(&0, 3);

			assert_eq!(OwnedKittiesTest::get(&(0, None)), Some(KittyLinkedItem::<Test> {
				prev: None,
				next: None,
			}));

			assert_eq!(OwnedKittiesTest::get(&(0, Some(1))), None);

			assert_eq!(OwnedKittiesTest::get(&(0, Some(2))), None);

			assert_eq!(OwnedKittiesTest::get(&(0, Some(2))), None);
		});
	}

	#[test]
	fn basic_setup_works() {
		with_externalities(&mut new_test_ext(), || {
			// Verify Initial Storage
			assert_eq!(KittyModule::kitties_count(), 0);
			assert!(KittyModule::kitty(0).is_none());
			assert_eq!(KittyModule::kitty_owner(0), None);
			assert_eq!(KittyModule::kitty_price(0), None);
			assert_eq!(Balances::free_balance(1), 10);
			assert_eq!(Balances::free_balance(2), 20);
		});
	}

	#[test]
	fn create_works() {
		with_externalities(&mut new_test_ext(), || {
			// Call Functions
			assert_ok!(KittyModule::create(Origin::signed(1)));
			// Verify Storage
			assert_eq!(KittyModule::kitties_count(), 1);
			assert!(KittyModule::kitty(0).is_some());
			assert_eq!(KittyModule::kitty_owner(0), Some(1));
			assert_eq!(KittyModule::kitty_price(0), None);
		});
	}

	#[test]
	fn create_handles_basic_errors() {
		with_externalities(&mut new_test_ext(), || {
			// Setup
			<KittiesCount<Test>>::put(u32::max_value());
			// Call Functions
			assert_noop!(KittyModule::create(Origin::signed(1)), "Kitties count overflow");
			// Verify Storage
			assert_eq!(KittyModule::kitties_count(), u32::max_value());
			assert!(KittyModule::kitty(0).is_none());
			assert_eq!(KittyModule::kitty_owner(0), None);
			assert_eq!(KittyModule::kitty_price(0), None);
		});
	}

	#[test]
	fn transfer_works() {
		with_externalities(&mut new_test_ext(), || {
			// Setup
			assert_ok!(KittyModule::create(Origin::signed(1)));
			// Call Functions
			assert_ok!(KittyModule::transfer(Origin::signed(1), 2, 0));
			// Verify Storage
			assert_eq!(KittyModule::kitties_count(), 1);
			assert!(KittyModule::kitty(0).is_some());
			assert_eq!(KittyModule::kitty_owner(0), Some(2));
			assert_eq!(KittyModule::kitty_price(0), None);
		});
	}

	#[test]
	fn transfer_handles_basic_errors() {
		with_externalities(&mut new_test_ext(), || {
			// Setup
			assert_ok!(KittyModule::create(Origin::signed(1)));
			// Call Functions
			assert_noop!(KittyModule::transfer(Origin::signed(2), 2, 0), "Only owner can transfer kitty");
			assert_noop!(KittyModule::transfer(Origin::signed(1), 2, 1), "Only owner can transfer kitty");
			// Verify Storage
			assert_eq!(KittyModule::kitties_count(), 1);
			assert!(KittyModule::kitty(0).is_some());
			assert_eq!(KittyModule::kitty_owner(0), Some(1));
			assert_eq!(KittyModule::kitty_price(0), None);
		});
	}

	#[test]
	fn breed_works() {
		with_externalities(&mut new_test_ext(), || {
			// Setup
			assert_ok!(KittyModule::create(Origin::signed(1)));
			assert_ok!(KittyModule::create(Origin::signed(1)));
			// Call Functions
			assert_ok!(KittyModule::breed(Origin::signed(1), 0, 1));
			// Verify Storage
			assert_eq!(KittyModule::kitties_count(), 3);
			assert!(KittyModule::kitty(0).is_some());
			assert!(KittyModule::kitty(1).is_some());
			assert!(KittyModule::kitty(2).is_some());
			assert_eq!(KittyModule::kitty_owner(0), Some(1));
			assert_eq!(KittyModule::kitty_owner(1), Some(1));
			assert_eq!(KittyModule::kitty_owner(2), Some(1));
			assert_eq!(KittyModule::kitty_price(0), None);
			assert_eq!(KittyModule::kitty_price(1), None);
			assert_eq!(KittyModule::kitty_price(2), None);
		});
	}

	#[test]
	fn breed_handles_basic_errors() {
		with_externalities(&mut new_test_ext(), || {
			// Setup
			assert_ok!(KittyModule::create(Origin::signed(1)));
			assert_ok!(KittyModule::create(Origin::signed(2)));
			// Call Functions
			assert_noop!(KittyModule::breed(Origin::signed(1), 0, 0), "Needs different parent");
			assert_noop!(KittyModule::breed(Origin::signed(2), 0, 1), "Not owner of kitty1");
			assert_noop!(KittyModule::breed(Origin::signed(1), 0, 1), "Not owner of kitty2");
			assert_noop!(KittyModule::breed(Origin::signed(1), 2, 1), "Invalid kitty_id_1");
			assert_noop!(KittyModule::breed(Origin::signed(1), 0, 2), "Invalid kitty_id_2");
			// Verify Storage
			assert_eq!(KittyModule::kitties_count(), 2);
			assert!(KittyModule::kitty(0).is_some());
			assert!(KittyModule::kitty(1).is_some());
			assert!(KittyModule::kitty(2).is_none());
			assert_eq!(KittyModule::kitty_owner(0), Some(1));
			assert_eq!(KittyModule::kitty_owner(1), Some(2));
			assert_eq!(KittyModule::kitty_price(0), None);
			assert_eq!(KittyModule::kitty_price(1), None);
		});
	}

	#[test]
	fn ask_works() {
		with_externalities(&mut new_test_ext(), || {
			// Setup
			assert_ok!(KittyModule::create(Origin::signed(1)));
			// Call Functions
			assert_ok!(KittyModule::ask(Origin::signed(1), 0, Some(10)));
			// Verify Storage
			assert_eq!(KittyModule::kitties_count(), 1);
			assert!(KittyModule::kitty(0).is_some());
			assert_eq!(KittyModule::kitty_owner(0), Some(1));
			assert_eq!(KittyModule::kitty_price(0), Some(10));
		});
	}

	#[test]
	fn buy_works() {
		with_externalities(&mut new_test_ext(), || {
			// Setup
			assert_ok!(KittyModule::create(Origin::signed(1)));
			assert_ok!(KittyModule::ask(Origin::signed(1), 0, Some(10)));
			// Call Functions
			assert_ok!(KittyModule::buy(Origin::signed(2), 0, 10));
			// Verify Storage
			assert_eq!(KittyModule::kitties_count(), 1);
			assert!(KittyModule::kitty(0).is_some());
			assert_eq!(KittyModule::kitty_owner(0), Some(2));
			assert_eq!(KittyModule::kitty_price(0), None);
			assert_eq!(Balances::free_balance(1), 20);
			assert_eq!(Balances::free_balance(2), 10);
		});
	}
}

