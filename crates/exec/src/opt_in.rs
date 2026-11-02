//! opt-in の印（代わりの実行を許すかどうか）の型。
//!
//! 下-2 の入口が経路を選ぶときに読む値で、上の層や利用側が組み立てて渡す。
//! 値を持つだけで、経路の選択そのものはここでは行わない。

/// 代わりの実行を許す opt-in の印。FB-15。
///
/// 既定（`Default`）は両方とも `false`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OptIn {
    /// GPU 上の代わりの実行（`Route::GpuEmulated`）を許す。
    pub allow_gpu_emulated: bool,
    /// CPU での代わりの実行（`Route::Cpu`）を許す。
    pub allow_cpu: bool,
}

#[cfg(test)]
mod tests {
    use super::OptIn;
    use std::collections::HashSet;

    #[test]
    fn default_is_all_false() {
        let d = OptIn::default();
        assert!(!d.allow_gpu_emulated);
        assert!(!d.allow_cpu);
    }

    #[test]
    fn four_combinations_are_distinct() {
        let all = [
            OptIn {
                allow_gpu_emulated: false,
                allow_cpu: false,
            },
            OptIn {
                allow_gpu_emulated: true,
                allow_cpu: false,
            },
            OptIn {
                allow_gpu_emulated: false,
                allow_cpu: true,
            },
            OptIn {
                allow_gpu_emulated: true,
                allow_cpu: true,
            },
        ];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                assert_eq!(i == j, a == b);
            }
        }
        let set: HashSet<OptIn> = all.into_iter().collect();
        assert_eq!(set.len(), 4);
    }
}
