macro_rules! debug {
    ($fmt:expr) => {
      if cfg!(debug_assertions) {
        print!(concat!($fmt, "\n"));
      }
    };
    ($fmt:expr, $($arg:tt)*) => {
      if cfg!(debug_assertions) {
        print!(concat!($fmt, "\n"), $($arg)*)
      }
    };
}

macro_rules! retry {
  ($expr:expr, 0) => (try!($expr));
  ($expr:expr, $n:expr) => ({
    let result;
    let mut tries_left = $n;
    loop {
      tries_left -= 1;
      let test = $expr;

      if let Ok(res) = test {
        result = res;
        break;
      }

      if tries_left == 0 {
        return Err(From::from(test.unwrap_err()));
      }

      let jitter = thread_rng().gen_range::<u64>(0, 500);
      thread::sleep(Duration::from_millis(1000 + jitter));
    }
    result
  });
}
