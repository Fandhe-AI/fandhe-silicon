//! 背後実装と機材の呼び名、およびその組み合わせの固定の列挙。
//!
//! 試験の宣言（`declaration`）と実行の設定（`setting`）の両方がこの列挙を共有する。
//! `core` は backend の feature・`cfg` を持たないので、ここは名前を表すだけで device には触れない。

/// 試験が対象にする背後実装の名前。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Backend {
    /// Metal。
    Metal,
    /// Vulkan。
    Vulkan,
    /// CUDA。
    Cuda,
}

/// 機材の呼び名。別名があるものは別名、無いものは製品名の呼び名だけを使う。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Machine {
    /// Metal の実機（Mac mini）。
    MacMini,
    /// 別名 node-A。
    NodeA,
    /// 別名 node-B。
    NodeB,
    /// AMD の実機。
    Amd,
    /// Intel の実機。
    Intel,
}

impl Machine {
    /// 呼び名を ASCII の小文字・数字・ハイフンで返す。
    pub const fn name(self) -> &'static str {
        match self {
            Machine::MacMini => "mac-mini",
            Machine::NodeA => "node-a",
            Machine::NodeB => "node-b",
            Machine::Amd => "amd",
            Machine::Intel => "intel",
        }
    }
}

/// 背後実装と機材の組み合わせ。値は固定で、実行の設定の名前（[`MachineCombination::name`]）と 1 対 1 に対応する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MachineCombination {
    /// Metal と Mac mini。
    MetalMacMini,
    /// Vulkan と node-B。
    VulkanNodeB,
    /// Vulkan と node-A。
    VulkanNodeA,
    /// Vulkan と AMD の実機。
    VulkanAmd,
    /// Vulkan と Intel の実機。
    VulkanIntel,
    /// CUDA と node-B。
    CudaNodeB,
    /// CUDA と node-A。
    CudaNodeA,
}

impl MachineCombination {
    /// 全ての組み合わせ。重複は無い。
    pub const ALL: [MachineCombination; 7] = [
        MachineCombination::MetalMacMini,
        MachineCombination::VulkanNodeB,
        MachineCombination::VulkanNodeA,
        MachineCombination::VulkanAmd,
        MachineCombination::VulkanIntel,
        MachineCombination::CudaNodeB,
        MachineCombination::CudaNodeA,
    ];

    /// 組み合わせの背後実装。
    pub const fn backend(self) -> Backend {
        match self {
            MachineCombination::MetalMacMini => Backend::Metal,
            MachineCombination::VulkanNodeB
            | MachineCombination::VulkanNodeA
            | MachineCombination::VulkanAmd
            | MachineCombination::VulkanIntel => Backend::Vulkan,
            MachineCombination::CudaNodeB | MachineCombination::CudaNodeA => Backend::Cuda,
        }
    }

    /// 組み合わせの機材。
    pub const fn machine(self) -> Machine {
        match self {
            MachineCombination::MetalMacMini => Machine::MacMini,
            MachineCombination::VulkanNodeB | MachineCombination::CudaNodeB => Machine::NodeB,
            MachineCombination::VulkanNodeA | MachineCombination::CudaNodeA => Machine::NodeA,
            MachineCombination::VulkanAmd => Machine::Amd,
            MachineCombination::VulkanIntel => Machine::Intel,
        }
    }

    /// 環境変数で受け渡す名前。ASCII の小文字・数字・ハイフンだけ。
    pub const fn name(self) -> &'static str {
        match self {
            MachineCombination::MetalMacMini => "metal-mac-mini",
            MachineCombination::VulkanNodeB => "vulkan-node-b",
            MachineCombination::VulkanNodeA => "vulkan-node-a",
            MachineCombination::VulkanAmd => "vulkan-amd",
            MachineCombination::VulkanIntel => "vulkan-intel",
            MachineCombination::CudaNodeB => "cuda-node-b",
            MachineCombination::CudaNodeA => "cuda-node-a",
        }
    }

    /// 名前から組み合わせを引く。完全一致だけを受け、空白の除去や大文字小文字の読み替えはしない。
    pub fn from_name(name: &str) -> Option<MachineCombination> {
        MachineCombination::ALL
            .into_iter()
            .find(|c| c.name() == name)
    }
}
