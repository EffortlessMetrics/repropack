// Feature: repropack-v02-alpha, Property 2: Replay environment baseline contains only declared variables
// Feature: repropack-v02-alpha, Property 3: Environment classification is a disjoint partition

use proptest::prelude::*;
use repropack_replay::{build_env_baseline, classify_env};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// Generate a BTreeMap<String, String> of env vars with valid env-var-style keys.
fn arb_env_vars() -> impl Strategy<Value = BTreeMap<String, String>> {
    prop::collection::btree_map("[A-Z_]{1,8}", "[a-zA-Z0-9_/]{0,16}", 0..8)
}

proptest! {
    /// **Validates: Requirements 4.1, 4.2, 4.3**
    ///
    /// Property 2: For any allowed_vars map and set_env override map,
    /// when replay constructs the command environment without --inherit-env,
    /// the resulting environment shall contain exactly the union of
    /// allowed_vars keys and set_env keys, with set_env values taking
    /// precedence for keys present in both.
    #[test]
    fn replay_env_baseline_contains_only_declared_variables(
        allowed_vars in arb_env_vars(),
        set_env in arb_env_vars(),
        host_env in arb_env_vars(),
    ) {
        let result = build_env_baseline(&allowed_vars, &set_env, &host_env, false);

        // The key set must be exactly the union of allowed_vars keys and set_env keys
        let expected_keys: BTreeSet<&String> =
            allowed_vars.keys().chain(set_env.keys()).collect();
        let actual_keys: BTreeSet<&String> = result.keys().collect();
        prop_assert_eq!(
            &expected_keys, &actual_keys,
            "env baseline keys must be exactly union of allowed_vars and set_env keys"
        );

        // For keys in set_env, the value must come from set_env (set_env wins)
        for (key, value) in &set_env {
            prop_assert_eq!(
                result.get(key).unwrap(),
                value,
                "set_env value must take precedence for key {:?}", key
            );
        }

        // For keys only in allowed_vars (not in set_env), value must come from allowed_vars
        for (key, value) in &allowed_vars {
            if !set_env.contains_key(key) {
                prop_assert_eq!(
                    result.get(key).unwrap(),
                    value,
                    "allowed_vars value must be used for key {:?} not in set_env", key
                );
            }
        }
    }
}

proptest! {
    /// **Validates: Requirements 5.1, 5.2**
    ///
    /// Property 3: For any replay producing an EnvClassification, the restored,
    /// overridden, and inherited arrays shall be pairwise disjoint, and their
    /// union shall equal the complete set of env var keys injected into the
    /// replay command. A key present in both allowed_vars and set_env shall
    /// appear in overridden and not in restored.
    #[test]
    fn env_classification_is_disjoint_partition(
        allowed_vars in arb_env_vars(),
        set_env in arb_env_vars(),
        host_env in arb_env_vars(),
        inherit_env in any::<bool>(),
    ) {
        let final_env = build_env_baseline(&allowed_vars, &set_env, &host_env, inherit_env);
        let cls = classify_env(&allowed_vars, &set_env, &host_env, inherit_env);

        let restored: BTreeSet<&str> = cls.restored.iter().map(String::as_str).collect();
        let overridden: BTreeSet<&str> = cls.overridden.iter().map(String::as_str).collect();
        let inherited: BTreeSet<&str> = cls.inherited.iter().map(String::as_str).collect();

        // Pairwise disjoint
        prop_assert!(
            restored.is_disjoint(&overridden),
            "restored and overridden must be disjoint"
        );
        prop_assert!(
            restored.is_disjoint(&inherited),
            "restored and inherited must be disjoint"
        );
        prop_assert!(
            overridden.is_disjoint(&inherited),
            "overridden and inherited must be disjoint"
        );

        // Union equals the complete set of env var keys in the final env
        let mut union: BTreeSet<&str> = BTreeSet::new();
        union.extend(&restored);
        union.extend(&overridden);
        union.extend(&inherited);

        let final_keys: BTreeSet<&str> = final_env.keys().map(String::as_str).collect();
        prop_assert_eq!(
            &union, &final_keys,
            "union of classification arrays must equal final env keys"
        );

        // Keys in both allowed_vars and set_env must be in overridden, not restored
        for key in allowed_vars.keys() {
            if set_env.contains_key(key) {
                prop_assert!(
                    overridden.contains(key.as_str()),
                    "key in both allowed_vars and set_env must be overridden"
                );
                prop_assert!(
                    !restored.contains(key.as_str()),
                    "key in both allowed_vars and set_env must not be restored"
                );
            }
        }
    }
}
