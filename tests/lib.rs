#[cfg(test)]
mod unit_tests {

    // tokio_test would be needed for more advanced testing

    mod crypto_tests;
    mod incentive_tests;
    mod mesh_tests;
    mod protocol_tests;
    mod token_tests;
}

#[cfg(test)]
mod security;

#[cfg(test)]
mod gaming;

#[cfg(test)]
mod integration;
#![cfg(feature = "legacy-tests")]
