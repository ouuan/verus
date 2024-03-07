use crate::prelude::*;

verus!{

/// Interface for ghost state that is consistent with the common
/// presentations of PCMs / resource algebras.
///
/// For applications, I generally advise using the
/// [`tokenized_state_machine!` system](https://verus-lang.github.io/verus/state_machines/).
/// This interface may be more familiar to many people, and it may be
/// useful for academic purposes.

#[verifier::external_body]
#[verifier::reject_recursive_types_in_ground_variants(P)]
tracked struct Resource<P> {
    p: P,
}

type Loc = int;

pub trait PCM : Sized {
    spec fn valid(self) -> bool;
    spec fn op(self, other: Self) -> Self;

    proof fn closed_under_incl(a: Self, b: Self)
        requires Self::op(a, b).valid(),
        ensures a.valid();

    proof fn commutative(a: Self, b: Self)
        ensures Self::op(a, b) == Self::op(b, a);

    proof fn associative(a: Self, b: Self, c: Self)
        ensures Self::op(a, Self::op(b, c))
            == Self::op(Self::op(a, b), c);
}

pub trait UnitalPCM : PCM {
    spec fn unit() -> Self;

    proof fn op_unit(a: Self)
        ensures Self::op(a, Self::unit()) == a;

    proof fn unit_valid()
        ensures Self::valid(Self::unit());
}

pub open spec fn incl<P: PCM>(a: P, b: P) -> bool {
    exists |c| P::op(a, c) == b
}

pub open spec fn frame_preserving_update<P: PCM>(a: P, b: P) -> bool {
    forall |c|
        #![trigger P::op(a, c), P::op(b, c)]
        P::op(a, c).valid() ==> P::op(b, c).valid()
}

impl<P: PCM> Resource<P> {
    pub open spec fn value(self) -> P;
    pub open spec fn loc(self) -> Loc;

    #[verifier::external_body]
    pub proof fn alloc(value: P) -> (tracked out: Self)
        requires value.valid(),
        ensures
            out.value() == value,
    { unimplemented!(); }

    #[verifier::external_body]
    pub proof fn join(tracked self, tracked other: Self) -> (tracked out: Self)
        requires self.loc() == other.loc(),
        ensures out.loc() == self.loc(),
            out.value() == P::op(self.value(), other.value()),
    { unimplemented!(); }

    #[verifier::external_body]
    pub proof fn split(tracked self, left: P, right: P) -> (tracked out: (Self, Self))
        requires self.value() == P::op(left, right),
        ensures
            out.0.loc() == self.loc(),
            out.1.loc() == self.loc(),
            out.0.value() == left,
            out.1.value() == right,
    { unimplemented!(); }

    #[verifier::external_body]
    pub proof fn update(tracked self, new_value: P) -> (tracked out: Self)
        requires frame_preserving_update(self.value(), new_value)
        ensures out.loc() == self.loc(),
            out.value() == new_value,
    { unimplemented!(); }

    #[verifier::external_body]
    pub proof fn create_unit(loc: Loc) -> (tracked out: Self)
        where P: UnitalPCM
        ensures out.value() == P::unit(),
            out.loc() == loc,
    { unimplemented!(); }

    #[verifier::external_body]
    pub proof fn is_valid(tracked &self)
        ensures self.value().valid(),
    { unimplemented!(); }
}

}
