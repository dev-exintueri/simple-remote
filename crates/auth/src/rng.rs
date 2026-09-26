/// 암호에 쓰는 난수 발생기. `getrandom` 의 시스템 RNG 를 감싼다.
///
/// D 결정에 따라 `rand_core` 0.6 `OsRng` 는 쓰지 않는다.
pub fn rng() -> rand_core::UnwrapErr<getrandom::SysRng> {
    rand_core::UnwrapErr(getrandom::SysRng)
}
