use multipeerconnectivity::prelude::*;

fn main() {
    println!("MCError domain: {}", mc_error_domain());
    println!("TimedOut raw code: {}", MCErrorCode::TimedOut.as_raw());
}
