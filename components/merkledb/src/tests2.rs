
use crate::{Database, Fork, TemporaryDB, ProofMapIndex, hash::*};

const IDX_NAME: &str = "index";

#[test]
fn test_insert_trivial() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
    let storage1 = db1.fork();
    let storage2 = db2.fork();

    let mut index1 = ProofMapIndex::new(IDX_NAME, &storage1);
    index1.put(&[255; 32], vec![1]);
    index1.put(&[254; 32], vec![2]);

    let mut index2 = ProofMapIndex::new(IDX_NAME, &storage2);
    index2.put(&[254; 32], vec![2]);
    index2.put(&[255; 32], vec![1]);

    assert_eq!(index1.get(&[255; 32]), Some(vec![1]));
    assert_eq!(index1.get(&[254; 32]), Some(vec![2]));
    assert_eq!(index2.get(&[255; 32]), Some(vec![1]));
    assert_eq!(index2.get(&[254; 32]), Some(vec![2]));

    assert_ne!(index1.object_hash(), HashTag::empty_map_hash());
    assert_eq!(index1.object_hash(), index2.object_hash());
}