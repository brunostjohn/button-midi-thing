pub fn compose_loop<F>(fns: &[F])
where
    F: Fn(),
{
    for f in fns {
        f();
    }
}
