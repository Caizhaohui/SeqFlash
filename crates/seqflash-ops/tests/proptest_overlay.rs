//! Property tests for random overlay operation sequences (M8 / plan §26.3).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use seqflash_ops::{EditOverlay, RecordEdit};

#[derive(Clone, Debug)]
enum Op {
    Delete(u64),
    Replace(u64),
    InsertBefore(u64),
    InsertAfter(u64),
    Undo,
    Redo,
}

fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u64..32).prop_map(Op::Delete),
        (0u64..32).prop_map(Op::Replace),
        (0u64..32).prop_map(Op::InsertBefore),
        (0u64..32).prop_map(Op::InsertAfter),
        Just(Op::Undo),
        Just(Op::Redo),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Random overlay ops never panic and revision only moves forward or stays.
    #[test]
    fn random_overlay_ops_stable(ops in prop::collection::vec(arb_op(), 0..40)) {
        let mut ov = EditOverlay::new();
        let mut last_rev = ov.revision().get();
        for op in ops {
            match op {
                Op::Delete(n) => ov.apply(RecordEdit::Delete { record_number: n }),
                Op::Replace(n) => ov.apply(RecordEdit::Replace {
                    record_number: n,
                    data: b">x\nAC\n".to_vec(),
                }),
                Op::InsertBefore(n) => ov.apply(RecordEdit::InsertBefore {
                    record_number: n,
                    data: b">i\nTT\n".to_vec(),
                }),
                Op::InsertAfter(n) => ov.apply(RecordEdit::InsertAfter {
                    record_number: n,
                    data: b">j\nGG\n".to_vec(),
                }),
                Op::Undo => {
                    let _ = ov.undo();
                }
                Op::Redo => {
                    let _ = ov.redo();
                }
            }
            let r = ov.revision().get();
            // Failed undo/redo leave revision unchanged; successful ops bump it.
            // Never goes backwards.
            prop_assert!(r >= last_rev);
            last_rev = r;
            // edited_record_count is consistent with emptiness.
            if ov.is_dirty() {
                prop_assert!(ov.edited_record_count() > 0);
            } else {
                prop_assert_eq!(ov.edited_record_count(), 0);
            }
        }
    }
}
