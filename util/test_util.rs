pub fn assert_equal_vec_unordered<T: Eq + std::fmt::Debug>(vec1: Vec<T>, vec2: Vec<T>) {
    // Without assuming anything about T besides Eq and Debug (like Hash or Ord),
    // the best we can do is O(n^2). This is OK for tests. Please don't use this
    // for non-test code.
    assert_eq!(
        vec1.len(),
        vec2.len(),
        "Vectors have different lengths: {:?}, {:?}",
        vec1,
        vec2
    );
    'outer: for item1 in vec1.iter() {
        for item2 in vec2.iter() {
            if item1 == item2 {
                continue 'outer;
            }
        }
        assert!(
            false,
            "Vectors are not order-independent equal:\n  {:?}\n  {:?}",
            vec1, vec2
        );
    }
}
