#[test]
fn sf_routes_test() {
    let (state, graph) = sf_routes::setup();

    let mut success = true;
    for test in sf_routes::TESTS.iter() {
        println!("Computing best route for {}", test.name);

        let route = sf_routes::perform_query(&state, graph.borrow_mut(), test);

        if let Some(route) = route {
            let mut failed_predicates = Vec::new();
            for predicate in &test.predicates {
                if !predicate.eval(&route) {
                    println!("Test {} failed predicate {:?}", test.name, predicate);
                    failed_predicates.push(predicate.clone());
                }
            }

            if !failed_predicates.is_empty() {
                success = false;

                println!();
                println!(
                    "Test {} failed {} predicate(s)",
                    test.name,
                    failed_predicates.len()
                );
                println!();
                route.print();
                println!();
                println!("Failed predicates:");
                for predicate in failed_predicates {
                    println!("  {:?}", predicate);
                }
            }
        } else {
            success = false;
            println!("Test {} did not produce a route", test.name);
            println!();
        }
    }

    println!();
    println!();
    assert!(success, "Some tests failed");
}
