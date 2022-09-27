#[allow(unused_imports)]
use builtin::*;
use builtin_macros::*;
mod pervasive;
use pervasive::*;
use pervasive::vec::*;
use pervasive::modes::*;
use pervasive::multiset::*;
use pervasive::map::*;
use pervasive::seq::*;
use pervasive::option::*;
use pervasive::atomic_ghost::*;

use state_machines_macros::tokenized_state_machine;

tokenized_state_machine!{
    DistRwLock<T> {
        fields {
            #[sharding(constant)]
            pub rc_width: int,

            #[sharding(storage_option)]
            pub storage: Option<T>,

            #[sharding(variable)]
            pub exc_locked: bool,

            #[sharding(map)]
            pub ref_counts: Map<int, int>,

            #[sharding(option)]
            pub exc_pending: Option<int>,

            #[sharding(option)]
            pub exc_guard: Option<()>,

            #[sharding(multiset)]
            pub shared_pending: Multiset<int>,

            #[sharding(multiset)]
            pub shared_guard: Multiset<(int, T)>,
        }

        init!{
            initialize(rc_width: int, init_t: T) {
                require(0 < rc_width);
                init rc_width = rc_width;
                init storage = Option::Some(init_t);
                init exc_locked = false;
                init ref_counts = Map::new(
                    |i| 0 <= i < rc_width,
                    |i| 0,
                );
                init exc_pending = Option::None;
                init exc_guard = Option::None;
                init shared_pending = Multiset::empty();
                init shared_guard = Multiset::empty();
            }
        }

        transition!{
            exc_start() {
                require(!pre.exc_locked);
                update exc_locked = true;
                add exc_pending += Some(0);
            }
        }

        transition!{
            exc_check_count() {
                remove exc_pending -= Some(let r);
                have ref_counts >= [r => 0];

                add exc_pending += Some(r + 1);
            }
        }

        transition!{
            exc_finish() {
                remove exc_pending -= Some(pre.rc_width);
                add exc_guard += Some(());
                withdraw storage -= Some(let _);
            }
        }

        transition!{
            exc_release(t: T) {
                update exc_locked = false;
                remove exc_guard -= Some(());
                deposit storage += Some(t);
            }
        }

        transition!{
            shared_start(r: int) {
                remove ref_counts -= [r => let rc];
                add ref_counts += [r => rc + 1];
                add shared_pending += {r};
            }
        }

        transition!{
            shared_finish(r: int) {
                require(!pre.exc_locked);
                remove shared_pending -= {r};

                birds_eye let t = pre.storage.get_Some_0();
                add shared_guard += {(r, t)};
            }
        }

        transition!{
            shared_release(val: (int, T)) {
                remove shared_guard -= {val};

                let r = val.0;
                remove ref_counts -= [r => let rc];
                add ref_counts += [r => rc - 1];

                assert(rc > 0) by {
                    assert(0 <= r < pre.rc_width);
                    assert(pre.shared_guard.count(val) > 0);
                    assert(Self::filter_r(pre.shared_guard, r).count(val) > 0);
                    assert(Self::filter_r(pre.shared_guard, r).len() > 0);
                    assert(pre.ref_counts.index(r) > 0);
                };
            }
        }

        property!{
            do_guard(val: (int, T)) {
                have shared_guard >= {val};
                guard storage >= Some(val.1);
            }
        }

        ///// Invariants and proofs

        #[invariant]
        pub fn ref_counts_domain(&self) -> bool {
            &&& 0 < self.rc_width
            &&& forall |i: int| 0 <= i < self.rc_width <==> self.ref_counts.dom().contains(i)
        }

        #[invariant]
        pub fn exc_inv(&self) -> bool {
            &&& self.exc_locked <==> (self.exc_pending.is_Some() || self.exc_guard.is_Some())
            &&& self.storage.is_Some() <==> self.exc_guard.is_None()
            &&& if let Option::Some(cur_r) = self.exc_pending {
                &&& 0 <= cur_r <= self.rc_width
                &&& self.exc_guard.is_None()
                &&& forall |x| self.shared_guard.count(x) > 0 ==> !(0 <= x.0 < cur_r)
            } else {
                true
            }
        }

        #[invariant]
        pub fn shared_pending_in_range(&self) -> bool {
            forall |r| self.shared_pending.count(r) > 0 ==> (0 <= r < self.rc_width)
        }
        
        #[invariant]
        pub fn shared_guard_in_range(&self) -> bool {
            forall |x| self.shared_guard.count(x) > 0 ==> (0 <= x.0 < self.rc_width)
        }

        #[invariant]
        pub fn shared_inv_agree(&self) -> bool {
            forall |v| #[trigger] self.shared_guard.count(v) > 0 ==>
                self.storage === Option::Some(v.1)
        }

        pub closed spec fn filter_r(shared_guard: Multiset<(int, T)>, r: int) -> Multiset<(int, T)> {
            shared_guard.filter(|val| val.0 == r)
        }

        #[invariant]
        pub fn shared_counts_agree(&self) -> bool {
            forall |r| 0 <= r < self.rc_width ==>
                #[trigger] self.ref_counts.index(r) ==
                    self.shared_pending.count(r) as int +
                        Self::filter_r(self.shared_guard, r).len() as int
        }

        #[inductive(initialize)]
        fn initialize_inductive(post: Self, rc_width: int, init_t: T) {
            assert forall |r| 0 <= r < post.rc_width implies
                #[trigger] post.ref_counts.index(r) ==
                    post.shared_pending.count(r) as int +
                        Self::filter_r(post.shared_guard, r).len() as int
            by {
                assert(post.ref_counts.index(r) == 0);
                assert(post.shared_pending.count(r) == 0);
                assert_multisets_equal!(
                    Self::filter_r(post.shared_guard, r),
                    Multiset::empty(),
                );
                assert(Self::filter_r(post.shared_guard, r).len() == 0);
            }
            assert(post.shared_counts_agree());
        }
       
        #[inductive(exc_start)]
        fn exc_start_inductive(pre: Self, post: Self) {
            
        }
       
        #[inductive(exc_check_count)]
        fn exc_check_count_inductive(pre: Self, post: Self) {
            let prev_r = pre.exc_pending.get_Some_0();
            assert forall |x| #[trigger] post.shared_guard.count(x) > 0
                && x.0 == prev_r implies false
            by {
                assert(Self::filter_r(post.shared_guard, prev_r).count(x) > 0);
            }
        }
       
        #[inductive(exc_finish)]
        fn exc_finish_inductive(pre: Self, post: Self) {
        }
       
        #[inductive(exc_release)]
        fn exc_release_inductive(pre: Self, post: Self, t: T) {
            
        }
       
        #[inductive(shared_start)]
        fn shared_start_inductive(pre: Self, post: Self, r: int) { }
       
        #[inductive(shared_finish)]
        fn shared_finish_inductive(pre: Self, post: Self, r: int) {
            let t = pre.storage.get_Some_0();

            assert forall |r0| 0 <= r0 < post.rc_width implies
                #[trigger] post.ref_counts.index(r0) ==
                    post.shared_pending.count(r0) as int +
                        Self::filter_r(post.shared_guard, r0).len() as int
            by {
                if r == r0 {
                    assert_multisets_equal!(
                        pre.shared_pending,
                        post.shared_pending.add(Multiset::singleton(r))
                    );
                    assert_multisets_equal!(
                        Self::filter_r(post.shared_guard, r),
                        Self::filter_r(pre.shared_guard, r).add(
                            Multiset::singleton((r, t)))
                    );
                    assert(post.ref_counts.index(r0) ==
                        post.shared_pending.count(r0) as int +
                            Self::filter_r(post.shared_guard, r0).len() as int);
                } else {
                    assert_multisets_equal!(
                          Self::filter_r(post.shared_guard, r0),
                          Self::filter_r(pre.shared_guard, r0)
                    );
                    assert(post.ref_counts.index(r0) ==
                        post.shared_pending.count(r0) as int +
                            Self::filter_r(post.shared_guard, r0).len() as int);
                }
            }
        }
       
        #[inductive(shared_release)]
        fn shared_release_inductive(pre: Self, post: Self, val: (int, T)) {
            let r = val.0;
            assert forall |r0| 0 <= r0 < post.rc_width implies
                #[trigger] post.ref_counts.index(r0) ==
                    post.shared_pending.count(r0) as int +
                        Self::filter_r(post.shared_guard, r0).len() as int
            by {
                if r0 == r {
                    assert_multisets_equal!(
                        Self::filter_r(pre.shared_guard, r),
                        Self::filter_r(post.shared_guard, r).add(
                            Multiset::singleton(val))
                    );
                } else {
                    assert_multisets_equal!(
                          Self::filter_r(post.shared_guard, r0),
                          Self::filter_r(pre.shared_guard, r0)
                    );
                }
            }
        }
    }
}

verus!{

struct RwLock<T> {
    #[proof] inst: DistRwLock::Instance<T>,
    exc_locked: AtomicBool<DistRwLock::exc_locked<T>>,
    ref_counts: Vec<AtomicU64<DistRwLock::ref_counts<T>>>,
}

}

impl<T> RwLock<T> {
    verus!{

    spec fn wf(&self) -> bool {
        &&& self.inst.rc_width() == self.ref_counts.view().len()
        &&& self.exc_locked.has_inv(|b: bool, g: DistRwLock::exc_locked<T>| g@ === DistRwLock::token![ self.inst => exc_locked => b ])
        &&& forall |i: int| (0 <= i && i < self.ref_counts.view().len()) ==>
            self.ref_counts@.index(i).has_inv(|r: u64, g: DistRwLock::ref_counts<T>|
                g@ === DistRwLock::token![ self.inst => ref_counts => i => r ])
    }

    }

    #[verifier(spinoff_prover)] 
    fn new(rc_width: usize, t: T) -> Self {
        requires(0 < rc_width);
        ensures(|s: Self| s.wf());
        
        #[proof] let (Trk(inst), Trk(exc_locked_token), Trk(mut ref_counts_tokens), _, _, _, _) =
            DistRwLock::Instance::initialize(rc_width, t, Option::Some(t));

        let exc_locked_atomic = AtomicBool::new(false, exc_locked_token,
            |b, g| equal(g.view(), DistRwLock::token![ inst => exc_locked => b ])
        );

        let mut v: Vec<AtomicU64<DistRwLock::ref_counts<T>>> = Vec::new();
        let mut i: usize = 0;

        assert_forall_by(|j: int| {
            requires(i <= j && j < rc_width);
            ensures(#[trigger] ref_counts_tokens.dom().contains(j)
                  && equal(ref_counts_tokens.index(j).view().instance, inst)
                  && equal(ref_counts_tokens.index(j).view().key, j)
                  && equal(ref_counts_tokens.index(j).view().value, 0));

            assert(ref_counts_tokens.dom().contains(j));
            assert(equal(ref_counts_tokens.index(j).view().instance, inst));
            assert(equal(ref_counts_tokens.index(j).view().key, j));
            assert(equal(ref_counts_tokens.index(j).view().value, 0));
        });

        assert(forall(|j: int| with_triggers!(
            [ref_counts_tokens.dom().contains(j)],[ref_counts_tokens.index(j)] =>
            i <= j && j < rc_width >>= (
            ref_counts_tokens.dom().contains(j)
            && equal(ref_counts_tokens.index(j).view().instance, inst)
            && equal(ref_counts_tokens.index(j).view().key, j)
            && equal(ref_counts_tokens.index(j).view().value, 0)
        ))));

        while i < rc_width {
            invariant([
                i <= rc_width,
                v.view().len() == i as int,
                forall(|j: int| 0 <= j && j < i >>=
                    v.view().index(j).has_inv(|r: u64, g: DistRwLock::ref_counts<T>|
                        equal(g.view(), DistRwLock::token![ inst => ref_counts => j => r ]))
                ),
                forall(|j: int| with_triggers!(
                    [ref_counts_tokens.dom().contains(j)],[ref_counts_tokens.index(j)] =>
                    i <= j && j < rc_width >>= (
                    ref_counts_tokens.dom().contains(j)
                    && equal(ref_counts_tokens.index(j).view().instance, inst)
                    && equal(ref_counts_tokens.index(j).view().key, j)
                    && equal(ref_counts_tokens.index(j).view().value, 0)
                ))),
            ]);

            assert(ref_counts_tokens.dom().contains(i));

            #[proof] let ref_count_token = ref_counts_tokens.tracked_remove(i as int);

            let rc_atomic = AtomicU64::new(0, ref_count_token,
                |r: u64, g| equal(g.view(), DistRwLock::token![ inst => ref_counts => i => r ]));
            v.push(rc_atomic);

            i = i + 1;

            assert_forall_by(|j: int| {
                requires(i <= j && j < rc_width);
                ensures(#[trigger] ref_counts_tokens.dom().contains(j)
                      && equal(ref_counts_tokens.index(j).view().instance, inst)
                      && equal(ref_counts_tokens.index(j).view().key, j)
                      && equal(ref_counts_tokens.index(j).view().value, 0));

                assert(ref_counts_tokens.dom().contains(j));
                assert(equal(ref_counts_tokens.index(j).view().instance, inst));
                assert(equal(ref_counts_tokens.index(j).view().key, j));
                assert(equal(ref_counts_tokens.index(j).view().value, 0));
            });
        }

        let s = RwLock {
            inst,
            exc_locked: exc_locked_atomic,
            ref_counts: v,
        };

        assert(s.inst.rc_width() == s.ref_counts.view().len());
        assert(s.exc_locked.has_inv(|b: bool, g: DistRwLock::exc_locked<T>|
            equal(g.view(), DistRwLock::token![ s.inst => exc_locked => b ])));
        assert(forall(|i: int| { (0 <= i && i < s.ref_counts.view().len()) >>=
            s.ref_counts.view().index(i).has_inv(|r: u64, g: DistRwLock::ref_counts<T>|
                equal(g.view(), DistRwLock::token![ s.inst => ref_counts => i => r ]))
        }));

        s

    }
}

fn main() { }
