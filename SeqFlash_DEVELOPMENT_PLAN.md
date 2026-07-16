# SeqFlash Windows-first 独立开发计划

* **计划版本**：2.0
* **项目名称**：SeqFlash
* **项目类型**：Windows 桌面应用
* **主要语言**：Rust
* **目标平台**：Windows 10/11 x86-64
* **GUI 技术栈**：`eframe + egui`
* **Rust 工具链**：Stable MSVC
* **首个公开版本目标**：SeqFlash 1.0
* **核心文件格式**：FASTA、FASTQ
* **项目状态**：独立开发，不依赖 SeqBrio
* **主要开发环境**：个人 Windows 笔记本

---

# 1. 项目定位

## 1.1 一句话定位

> SeqFlash 是一个面向 Windows 的大型 FASTA/FASTQ 文件浏览器与轻量序列操作工具，重点解决数百 MB 至数 GB 生物序列文件的快速打开、流畅浏览、记录导航、搜索、检查和安全导出。

## 1.2 核心目标

SeqFlash 优先解决以下问题：

1. 普通文本编辑器打开大型 FASTA/FASTQ 文件速度慢。
2. 大型文件加载时占用大量内存。
3. 用户难以按 FASTA/FASTQ 记录浏览。
4. 用户难以快速定位指定 Record ID。
5. FASTQ 文件结构错误不容易发现。
6. 查看单条序列长度、GC、N 和质量值不方便。
7. 简单序列操作通常需要额外使用命令行工具。
8. Windows 平台缺少专注大型序列文件的轻量 GUI 工具。

## 1.3 产品原则

SeqFlash 的优先级固定为：

1. 文件安全
2. 稳定性
3. 大文件打开速度
4. 浏览流畅性
5. FASTA/FASTQ 解析正确性
6. 搜索和导航
7. 轻量操作
8. 界面美观
9. 其他格式和高级功能

---

# 2. 与 SeqBrio 的关系

## 2.1 当前阶段

SeqFlash 当前是完全独立的 Rust 项目。

当前禁止：

* 依赖 `seqbrio-core`
* 依赖 `seqbrio-formats`
* 调用 `seqbrio.exe`
* 复制 SeqBrio 内部模块
* 将 SeqBrio 仓库作为 Git Submodule
* 为 SeqBrio 编写适配器
* 为 SeqBrio 设计运行时集成
* 因未来集成而阻塞 SeqFlash 的开发

SeqFlash 必须能够独立完成：

* FASTA 检测和解析
* FASTQ 检测和解析
* 记录索引
* 搜索
* 格式检查
* 序列统计
* 轻量序列操作
* 文件导出和重建

## 2.2 未来可能的集成

未来版本可以考虑与 SeqBrio 对接，但不属于 SeqFlash 1.0 范围。

为了避免未来难以集成，当前只遵循以下一般性原则：

* GUI 不直接包含复杂算法。
* 格式解析和界面代码分离。
* 后台任务和 GUI 状态分离。
* 序列操作通过独立 crate 暴露 Rust API。
* 输入和输出以标准 FASTA/FASTQ 字节流为主。
* 不在代码中硬编码 SeqBrio 名称或路径。

当前不创建任何 SeqBrio 接口、适配器或占位实现。

---

# 3. 产品边界

## 3.1 SeqFlash 是什么

SeqFlash 是：

* 大型序列文件查看器
* FASTA/FASTQ 记录浏览器
* Record ID 搜索工具
* 序列片段搜索工具
* 文件结构检查工具
* 当前记录统计工具
* 记录级轻量操作工具
* 安全导出工具

## 3.2 SeqFlash 不是什么

SeqFlash 1.0 不是：

* Rust 版 Notepad++
* 通用文本编辑器
* 代码编辑器
* IDE
* 生物信息学工作流平台
* 完整命令行工具图形化界面
* 十六进制编辑器
* 数据库浏览器
* 多组学分析平台
* BAM/CRAM 浏览器
* 基因组注释浏览器

## 3.3 SeqFlash 1.0 明确不做

* LSP
* 编程语言语法高亮
* 代码补全
* 宏录制
* 插件市场
* Git 集成
* 多人协作
* 云同步
* SSH 或 FTP 文件编辑
* 任意大小文件的自由文本编辑
* 数 GB 文件的原地保存
* 任意范围正则替换
* BAM、CRAM、SAM、VCF、GFF、BED
* 网络数据库访问
* 普通 gzip 文件的随机编辑
* 自动更新服务
* 跨平台发布

在 Windows 版本稳定前，不进行 Linux 和 macOS 适配。

---

# 4. 目标用户

主要用户：

* 生物信息学研究人员
* 测序数据分析人员
* 实验室科研人员
* FASTA/FASTQ 数据检查人员
* Windows 平台生物信息学初学者
* 需要快速查看大型序列文件的用户

典型使用场景：

1. 双击打开 2 GB FASTA 文件并立即查看首屏。
2. 搜索指定染色体或序列 Record ID。
3. 查看某条序列的长度和 GC 含量。
4. 检查 FASTQ 是否存在截断记录。
5. 查看 Sequence 和 Quality 长度是否一致。
6. 导出指定记录。
7. 删除少量记录并生成新文件。
8. 对当前序列执行 reverse complement。
9. 从大型文件中提取若干 ID。
10. 搜索一段 DNA 序列出现的位置。

---

# 5. 第一阶段用户体验

用户打开大型文件后的理想流程：

```text
双击 FASTA/FASTQ 文件
        ↓
SeqFlash 启动
        ↓
读取文件元数据和头部样本
        ↓
立即显示首屏内容
        ↓
后台识别格式并建立记录索引
        ↓
状态栏显示索引进度
        ↓
用户可以立即滚动和浏览
        ↓
索引完成后启用完整记录导航
        ↓
用户搜索、检查或导出记录
```

“文件打开完成”的定义是：

> 用户已经看到首屏并可以开始浏览。

不得将“完整文件索引完成”作为显示文件内容的前置条件。

---

# 6. 技术选型

## 6.1 Rust 工具链

使用：

```text
stable-x86_64-pc-windows-msvc
```

禁止将 GNU 工具链作为主要 Windows 构建目标。

开发和发布均以 MSVC 工具链为准。

## 6.2 GUI 框架

使用：

```toml
eframe
egui
```

选择原因：

* 完整 Rust 技术栈
* 不依赖 WebView
* 适合 Windows 桌面开发
* 支持自定义控件和绘制
* 适合实现虚拟列表
* 适合快速迭代
* 容易实现深色和浅色主题

不得在 SeqFlash 1.0 开发期间擅自更换 GUI 框架。

## 6.3 推荐依赖

```toml
eframe
egui
memmap2
memchr
aho-corasick
regex
crossbeam-channel
parking_lot
rayon
thiserror
anyhow
tracing
tracing-subscriber
serde
serde_json
directories
rfd
smallvec
bitflags
tempfile
```

测试和性能工具：

```toml
proptest
criterion
insta
```

模糊测试：

```text
cargo-fuzz
```

不强制一次性加入全部依赖。每个里程碑只添加实际需要的依赖。

---

# 7. 总体架构

## 7.1 分层架构

```text
┌─────────────────────────────────────────┐
│ seqflash-app                            │
│ Windows GUI、菜单、窗口、标签页、状态    │
├─────────────────────────────────────────┤
│ seqflash-viewer                         │
│ 虚拟滚动、文本绘制、选择、跳转           │
├─────────────────────────────────────────┤
│ seqflash-document                       │
│ 文件生命周期、mmap、文档状态、Overlay    │
├─────────────────────────────────────────┤
│ seqflash-index                          │
│ FASTA/FASTQ 记录索引、行检查点           │
├─────────────────────────────────────────┤
│ seqflash-search                         │
│ ID 搜索、字节搜索、序列搜索              │
├─────────────────────────────────────────┤
│ seqflash-formats                        │
│ FASTA/FASTQ 检测、解析、验证             │
├─────────────────────────────────────────┤
│ seqflash-ops                            │
│ 统计、反向互补、wrap、导出、过滤         │
├─────────────────────────────────────────┤
│ seqflash-jobs                           │
│ 后台任务、进度、取消、消息传递           │
├─────────────────────────────────────────┤
│ seqflash-platform-windows               │
│ Windows 路径、文件关联、DPI、系统集成    │
└─────────────────────────────────────────┘
```

## 7.2 依赖方向

允许的主要依赖方向：

```text
seqflash-app
    ↓
seqflash-viewer
seqflash-document
seqflash-search
seqflash-jobs
seqflash-settings
    ↓
seqflash-index
seqflash-formats
seqflash-ops
    ↓
seqflash-types
```

禁止：

* `seqflash-formats` 依赖 GUI。
* `seqflash-index` 依赖 GUI。
* `seqflash-ops` 依赖 GUI。
* `seqflash-viewer` 实现 FASTQ 解析。
* `seqflash-app` 直接扫描完整文件。
* crate 之间形成循环依赖。

---

# 8. 推荐仓库结构

```text
SeqFlash/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── DEVELOPMENT_PLAN.md
├── LICENSE
├── rustfmt.toml
├── deny.toml
├── .gitignore
├── .github/
│   └── workflows/
│       └── windows.yml
├── apps/
│   └── seqflash-app/
│       ├── Cargo.toml
│       ├── build.rs
│       ├── assets/
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── commands.rs
│           ├── state.rs
│           ├── tabs.rs
│           ├── menu.rs
│           ├── panels/
│           └── dialogs/
├── crates/
│   ├── seqflash-types/
│   ├── seqflash-document/
│   ├── seqflash-formats/
│   ├── seqflash-index/
│   ├── seqflash-viewer/
│   ├── seqflash-search/
│   ├── seqflash-ops/
│   ├── seqflash-jobs/
│   ├── seqflash-settings/
│   └── seqflash-platform-windows/
├── benches/
│   ├── indexing/
│   ├── searching/
│   ├── parsing/
│   └── rendering/
├── fuzz/
├── test-data/
│   ├── fasta/
│   ├── fastq/
│   ├── malformed/
│   └── expected/
├── scripts/
│   ├── generate-large-fasta.ps1
│   ├── generate-large-fastq.ps1
│   ├── benchmark.ps1
│   └── package-portable.ps1
└── docs/
    ├── architecture/
    ├── adr/
    ├── formats/
    └── performance/
```

---

# 9. 核心 crate 职责

## 9.1 `seqflash-types`

保存跨模块共享的基础类型：

```rust
pub struct DocumentId(pub u64);

pub struct JobId(pub u64);

pub struct Revision(pub u64);

pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

pub enum SequenceFormat {
    Fasta,
    Fastq,
    Unknown,
}

pub enum NewlineStyle {
    Lf,
    CrLf,
    Mixed,
    Unknown,
}
```

该 crate 不包含：

* GUI
* 文件 I/O
* FASTA/FASTQ 扫描
* 后台线程
* Windows API

## 9.2 `seqflash-document`

负责：

* 文件打开和关闭
* 只读内存映射
* 文件元数据
* 文档 ID
* 文档 Revision
* 文件变化检测
* 视口缓存
* 编辑 Overlay
* 生命周期管理

核心结构示例：

```rust
pub struct SequenceDocument {
    pub id: DocumentId,
    pub path: PathBuf,
    pub file_size: u64,
    pub modified_time: SystemTime,
    pub format: SequenceFormat,
    pub newline: NewlineStyle,
    pub revision: Revision,
    pub mmap: Arc<Mmap>,
    pub index_state: IndexState,
    pub edit_overlay: EditOverlay,
}
```

`memmap2` 所需的 `unsafe` 必须封装在该 crate 内部。

其他 crate 不得随意创建内存映射。

## 9.3 `seqflash-formats`

负责：

* 格式检测
* FASTA 记录解析
* FASTQ 状态机解析
* Header 和 ID 提取
* 换行处理
* 非法格式报告
* 局部记录验证
* 完整文件验证

不得假设：

* 文件一定是 UTF-8
* FASTA 序列一定单行
* FASTQ 一定严格四行
* 文件一定以换行结束
* 文件只包含 LF
* Header 一定包含可打印 ASCII

## 9.4 `seqflash-index`

负责：

* FASTA 记录边界索引
* FASTQ 记录边界索引
* Record ID 范围
* 行检查点
* 记录到文件偏移映射
* 文件偏移到记录映射
* 索引进度
* 索引取消
* 索引错误位置

## 9.5 `seqflash-viewer`

负责：

* 只绘制可见内容
* 虚拟滚动
* 原始文本视图
* 记录视图
* 字节偏移到屏幕位置映射
* 屏幕位置到字节偏移映射
* 选择和复制
* 搜索结果高亮
* 当前记录高亮
* 序列坐标显示

不得：

* 将整个文件交给 `egui::TextEdit`
* 为全部物理行创建 Widget
* 在绘制函数中扫描完整文件
* 在每帧重新解析完整记录

## 9.6 `seqflash-search`

负责：

* 普通字节搜索
* Record ID 精确搜索
* Record ID 前缀搜索
* 序列片段搜索
* 当前记录搜索
* 从当前位置继续搜索
* 搜索结果限制
* 搜索进度
* 任务取消

## 9.7 `seqflash-ops`

负责：

* 序列长度
* GC 含量
* N 数量
* 碱基组成
* reverse complement
* 大小写转换
* wrap
* unwrap
* FASTQ 质量统计
* 当前记录导出
* 多记录导出
* 按 ID 提取
* 按长度过滤
* FASTQ 转 FASTA

这些能力必须与 GUI 分离，并可通过单元测试独立验证。

## 9.8 `seqflash-jobs`

负责：

* 后台任务启动
* 状态管理
* 进度报告
* 取消令牌
* 线程通信
* 任务错误
* 任务完成事件
* 文档 Revision 检查

## 9.9 `seqflash-settings`

负责：

* 用户设置
* 最近文件
* 窗口位置
* 主题
* 字体
* 缓存参数
* 搜索结果上限
* 后台线程数量
* 配置序列化

## 9.10 `seqflash-platform-windows`

负责：

* Windows 文件路径
* 长路径支持
* 中文路径支持
* 文件关联
* 应用数据目录
* DPI 相关集成
* 单实例支持
* Windows 错误转换
* 后续安装器辅助能力

---

# 10. 大文件数据模型

## 10.1 字节优先

文件底层模型必须使用：

```rust
&[u8]
u64
ByteRange
```

不得使用一个大型 `String` 保存整个文件。

原因：

* 文件可能不是有效 UTF-8。
* FASTA/FASTQ 主体通常是 ASCII。
* 字节偏移需要对应真实磁盘位置。
* 搜索无需进行 Unicode 转换。
* 可以避免不必要的内存复制。
* 可以处理超过 4 GiB 的文件。

## 10.2 文件偏移

所有磁盘偏移和记录位置统一使用：

```rust
u64
```

只有在确认安全后才转换为 `usize`。

转换必须使用检查方式：

```rust
usize::try_from(offset)
```

禁止直接使用不安全的 `as usize` 处理文件偏移。

## 10.3 内存映射

第一阶段采用只读内存映射：

```text
File
  ↓
Mmap
  ↓
局部字节切片
  ↓
可见视口解码
  ↓
egui 绘制
```

不得：

* 将整个映射复制到 `Vec<u8>`
* 为每一行创建 `String`
* 将全部 Record ID 提前复制到内存
* 在文件打开时立即计算全部统计信息

---

# 11. 文件打开流程

```text
用户选择文件
    ↓
读取文件元数据
    ↓
检查文件是否为空
    ↓
建立只读 mmap
    ↓
读取头部样本
    ↓
检测换行格式
    ↓
初步判断 FASTA/FASTQ/Unknown
    ↓
创建 Document
    ↓
立即显示首屏
    ↓
启动后台索引
    ↓
增量更新记录数量和进度
```

文件打开失败时：

* 不得创建半初始化标签页。
* 不得导致应用退出。
* 应显示可读错误。
* 日志记录完整错误链。
* 不得泄漏文件句柄或 mmap。

---

# 12. 虚拟查看器设计

## 12.1 两种视图

### 原始文本视图

显示文件真实内容：

```text
>seq1 description
ATGCGTACGTACGT
ATGCGTACGTACGT
>seq2
ATGCGT
```

用途：

* 查看真实行包装
* 查看格式错误
* 查看 Header
* 复制原始内容
* 定位字节偏移

### 记录视图

按逻辑记录显示：

```text
ID: seq1
Description: description
Length: 28
GC: 50.0%

ATGCGTACGTACGTATGCGTACGTACGT
```

用途：

* 按记录浏览
* 查看统计
* 序列操作
* 当前记录修改
* FASTQ 质量显示

## 12.2 可见区域缓存

只缓存：

```text
当前视口
+ 上方少量缓冲
+ 下方少量缓冲
```

初始建议：

* 当前视口上方：200 行
* 当前视口下方：200 行
* 可配置最大缓存字节数
* 缓存按最近访问淘汰

缓存不得随滚动持续无限增长。

## 12.3 虚拟滚动

逻辑滚动位置使用：

```rust
pub struct ScrollPosition {
    pub byte_offset: u64,
    pub approximate_line: Option<u64>,
    pub record_index: Option<u64>,
}
```

滚动条使用归一化位置：

```text
0.0 ───────────────────────── 1.0
```

映射流程：

```text
滚动比例
  ↓
估算文件偏移
  ↓
寻找最近行检查点
  ↓
向前或向后局部扫描
  ↓
定位真实行边界
  ↓
绘制视口
```

---

# 13. FASTA 支持

## 13.1 FASTA 格式检测

初步检测规则：

* 忽略可选 BOM。
* 跳过开头空行。
* 第一条有效记录以 `>` 开头。
* 检查后续内容是否符合基本 FASTA 结构。
* 检测失败时标记为 `Unknown`，不得强制解析。

## 13.2 FASTA 索引

```rust
pub struct FastaRecordEntry {
    pub record_number: u64,
    pub start_offset: u64,
    pub end_offset: u64,
    pub header_range: ByteRange,
    pub id_range: ByteRange,
}
```

第一遍索引只记录：

* 记录编号
* 记录起始偏移
* 记录结束偏移
* Header 范围
* ID 范围

以下信息按需计算：

* 序列长度
* GC
* N 数量
* 非法字符
* 碱基组成

## 13.3 FASTA 当前记录统计

支持：

* ID
* Description
* 原始字节范围
* 序列长度
* A/C/G/T/U/N 数量
* GC%
* 非法字符数量
* 物理行数
* 是否存在空序列

## 13.4 FASTA 合法字符

默认允许：

```text
A C G T U
R Y S W K M
B D H V
N
-
.
```

大小写均允许。

字符检查策略必须可以配置。

---

# 14. FASTQ 支持

## 14.1 不采用固定四行假设

FASTQ 解析器必须使用状态机。

必须支持：

* 单行 Sequence
* 多行 Sequence
* 单行 Quality
* 多行 Quality
* CRLF
* LF
* 无文件末尾换行
* 截断文件
* 空序列
* 非法结构

## 14.2 FASTQ 状态机

建议状态：

```rust
pub enum FastqParserState {
    ExpectHeader,
    ReadSequence,
    ExpectPlus,
    ReadQuality,
    Complete,
    Error,
}
```

解析逻辑：

1. Header 必须以 `@` 开头。
2. Sequence 可以包含一行或多行。
3. Plus 行必须以 `+` 开头。
4. Quality 累计长度必须达到 Sequence 长度。
5. Quality 长度超过 Sequence 长度时报告错误。
6. 文件结束但 Quality 不完整时报告截断。
7. 不得因单条错误记录导致应用崩溃。

## 14.3 FASTQ 索引

```rust
pub struct FastqRecordEntry {
    pub record_number: u64,
    pub start_offset: u64,
    pub end_offset: u64,
    pub header_range: ByteRange,
    pub id_range: ByteRange,
    pub sequence_range: ByteRange,
    pub plus_range: ByteRange,
    pub quality_range: ByteRange,
    pub sequence_length: u64,
    pub quality_length: u64,
    pub validation: FastqValidation,
}
```

## 14.4 FASTQ 验证

第一阶段检查：

* Header 是否以 `@` 开头
* Plus 行是否存在
* Sequence 是否存在
* Sequence 和 Quality 长度是否一致
* Quality 是否截断
* Quality 字符是否在支持范围
* 文件末尾是否存在不完整记录
* 空记录
* 非法结构跳转位置

## 14.5 FASTQ 质量统计

当前记录支持：

* Sequence 长度
* Quality 长度
* 最低质量
* 最高质量
* 平均质量
* 低质量碱基数量
* Phred+33 显示

SeqFlash 1.0 不自动推断所有质量编码体系。

默认使用 Phred+33，并在界面中明确显示。

---

# 15. 记录导航

支持：

* 第一条记录
* 上一条记录
* 下一条记录
* 最后一条记录
* 输入记录编号跳转
* Record ID 搜索后跳转
* 错误记录跳转
* 搜索结果跳转
* 返回上一个位置
* 前进到下一个位置

导航历史示例：

```rust
pub struct NavigationEntry {
    pub document_id: DocumentId,
    pub byte_offset: u64,
    pub record_number: Option<u64>,
}
```

导航历史应限制最大数量，避免无限增长。

---

# 16. 搜索系统

## 16.1 SeqFlash 1.0 搜索类型

支持：

1. Record ID 精确搜索
2. Record ID 前缀搜索
3. 原始文本字节搜索
4. 序列片段搜索
5. 当前记录搜索
6. 从当前位置向后搜索
7. 上一个搜索结果
8. 下一个搜索结果

暂不支持：

* 全功能正则搜索
* 全文件正则替换
* 模糊编辑距离搜索
* 蛋白质 motif 数据库
* BLAST 类比对
* IUPAC 模糊匹配

## 16.2 搜索请求

```rust
pub struct SearchRequest {
    pub document_id: DocumentId,
    pub revision: Revision,
    pub mode: SearchMode,
    pub pattern: Vec<u8>,
    pub start_offset: u64,
    pub case_sensitive: bool,
    pub max_results: usize,
}
```

## 16.3 搜索结果

```rust
pub struct SearchResult {
    pub byte_range: ByteRange,
    pub record_number: Option<u64>,
    pub preview: Vec<u8>,
}
```

默认最多保留：

```text
10,000 条结果
```

达到上限后：

* 停止保存更多结果。
* 可以继续报告匹配总数为“至少 10,000”。
* 不得因结果过多耗尽内存。

## 16.4 搜索任务要求

所有全文件搜索必须：

* 后台运行
* 可取消
* 分块处理
* 定期报告进度
* 检查文档 Revision
* 不阻塞 GUI
* 不复制整个文件
* 允许尽早返回首个结果

---

# 17. 统计和检查

## 17.1 当前记录统计

FASTA：

* 序列长度
* GC%
* N 数量
* 碱基组成
* 非法字符
* 空序列检查

FASTQ：

* Sequence 长度
* Quality 长度
* 长度一致性
* 平均质量
* 最低质量
* 最高质量
* 低质量碱基数量

## 17.2 文件级统计

SeqFlash 1.0 后期支持：

FASTA：

* 记录数量
* 总序列长度
* 最短序列
* 最长序列
* 平均长度
* N50
* GC%
* 空记录数量
* 非法记录数量

FASTQ：

* 记录数量
* 总碱基数
* 最短长度
* 最长长度
* 平均长度
* 平均质量
* 结构错误数量
* 截断记录数量

文件级统计必须作为后台任务运行。

---

# 18. 轻量编辑模型

## 18.1 编辑范围

SeqFlash 不提供大型文件自由文本编辑。

SeqFlash 1.0 只支持记录级操作：

* 修改当前 Header
* 修改当前记录 Sequence
* 修改当前 FASTQ Quality
* 删除当前记录
* 替换当前记录
* 在记录前插入
* 在记录后插入
* reverse complement
* 大小写转换
* wrap
* unwrap

## 18.2 Overlay

原文件始终只读。

修改保存在 Overlay：

```rust
pub enum RecordEdit {
    Delete {
        record_number: u64,
    },
    Replace {
        record_number: u64,
        data: Vec<u8>,
    },
    InsertBefore {
        record_number: u64,
        data: Vec<u8>,
    },
    InsertAfter {
        record_number: u64,
        data: Vec<u8>,
    },
}
```

```rust
pub struct EditOverlay {
    pub edits: BTreeMap<u64, Vec<RecordEdit>>,
    pub revision: Revision,
}
```

## 18.3 当前记录编辑限制

为了避免内存风险，单条记录进入编辑模式前必须检查大小。

建议默认阈值：

```text
64 MiB
```

超过阈值时：

* 默认只读。
* 提示用户记录过大。
* 可以执行流式操作。
* 不允许直接加载到普通文本编辑控件。

阈值应可配置。

---

# 19. 序列操作

## 19.1 FASTA 操作

* reverse complement
* 转大写
* 转小写
* wrap
* unwrap
* 删除记录
* 替换记录
* 修改 Header
* 导出记录
* 按 ID 提取
* 按长度过滤

## 19.2 FASTQ 操作

* 修改 Header
* 删除记录
* 替换记录
* 导出记录
* FASTQ 转 FASTA
* 按 ID 提取
* 按长度过滤
* 复制 Sequence
* 复制 Quality

对 FASTQ 执行 reverse complement 时，必须同时：

* 反向互补 Sequence
* 反转 Quality

禁止只修改 Sequence 而不修改 Quality。

## 19.3 wrap 和 unwrap

FASTA wrap：

* 默认宽度 60 或 80
* 用户可配置
* 不修改 Header

FASTQ wrap：

* SeqFlash 1.0 默认不提供任意 wrap
* 避免破坏 Sequence 和 Quality 对应关系
* 后续版本可在严格验证后实现

---

# 20. 导出和保存

## 20.1 默认保存策略

SeqFlash 1.0 默认只提供：

> 另存为新文件

原文件不得直接覆盖。

## 20.2 流式重建

```text
创建目标目录临时文件
        ↓
顺序读取原始记录
        ↓
查询 Overlay
        ↓
写入原记录或替换内容
        ↓
写入插入记录
        ↓
flush
        ↓
检查任务状态
        ↓
原子重命名为目标文件
```

## 20.3 保存要求

保存必须：

* 后台执行
* 显示进度
* 支持取消
* 不把输出完整保存在内存
* 失败时删除临时文件
* 取消时删除临时文件
* 检查目标路径
* 检查磁盘空间错误
* 报告写入错误
* 保持源文件不变

## 20.4 源文件变化检查

开始保存前和保存完成前检查：

* 文件大小
* 修改时间
* 文件身份信息

发现源文件已被外部修改时：

* 停止保存。
* 不覆盖目标文件。
* 提示用户重新打开文件。
* 保留未保存 Overlay，供用户决定。

---

# 21. 后台任务系统

## 21.1 任务类型

```rust
pub enum JobKind {
    BuildIndex,
    ValidateFile,
    Search,
    ComputeStatistics,
    ExportRecords,
    RebuildFile,
    ConvertFastqToFasta,
}
```

## 21.2 任务结构

```rust
pub struct BackgroundJob {
    pub id: JobId,
    pub document_id: DocumentId,
    pub input_revision: Revision,
    pub kind: JobKind,
    pub status: JobStatus,
    pub progress: Option<f32>,
    pub cancel_token: CancellationToken,
}
```

## 21.3 任务状态

```rust
pub enum JobStatus {
    Queued,
    Running,
    CancelRequested,
    Cancelled,
    Completed,
    Failed,
}
```

## 21.4 Revision 规则

后台任务完成时必须验证：

```text
任务开始时的 Revision
            ==
当前文档 Revision
```

若不相同：

* 不自动覆盖当前结果。
* 将任务结果标记为过期。
* 必要时重新启动任务。
* 不得让旧任务破坏新状态。

---

# 22. GUI 设计

## 22.1 主界面

```text
┌───────────────────────────────────────────────────────────────┐
│ 文件  搜索  记录  序列操作  视图  工具  设置                  │
├───────────────────────────────────────────────────────────────┤
│ sample.fasta ×   reads.fastq ×                               │
├───────────────┬───────────────────────────────┬───────────────┤
│ 记录导航      │ 文件/记录查看区域             │ 信息面板      │
│               │                               │               │
│ 搜索 ID       │ >chr1 description             │ 文件格式      │
│ chr1          │ ATGCGT...                     │ 记录长度      │
│ chr2          │                               │ GC            │
│ chr3          │                               │ N             │
│ ...           │                               │ 验证状态      │
├───────────────┴───────────────────────────────┴───────────────┤
│ FASTA | 4.2 GB | Indexing 63% | Record 1 / 28,341           │
└───────────────────────────────────────────────────────────────┘
```

## 22.2 主界面区域

### 顶部菜单

* 文件
* 搜索
* 记录
* 序列操作
* 视图
* 工具
* 设置
* 帮助

### 标签页

* 多文件标签
* 未保存状态
* 索引进度状态
* 文件错误状态
* 关闭按钮

### 左侧记录面板

* Record ID 搜索框
* 记录列表
* 错误记录过滤
* 上一条/下一条
* 记录编号跳转

### 中央查看区

* 原始文本视图
* 记录视图
* 搜索高亮
* 文本选择
* 可见区域虚拟化

### 右侧信息面板

* 文件信息
* 当前记录信息
* 当前记录统计
* 验证结果
* 操作按钮

### 底部状态栏

* 文件格式
* 文件大小
* 当前偏移
* 当前记录
* 记录总数
* 索引状态
* 后台任务状态
* 修改状态

---

# 23. 设置项

```rust
pub struct AppSettings {
    pub theme: Theme,
    pub font_family: String,
    pub font_size: f32,
    pub sequence_wrap_width: usize,
    pub viewer_cache_lines: usize,
    pub viewer_cache_bytes: usize,
    pub max_search_results: usize,
    pub worker_threads: usize,
    pub record_edit_limit_bytes: u64,
    pub default_export_directory: Option<PathBuf>,
    pub reopen_previous_session: bool,
}
```

SeqFlash 1.0 设置页面支持：

* 浅色主题
* 深色主题
* 字体
* 字体大小
* 序列显示宽度
* 搜索结果上限
* 后台线程数
* 单条记录编辑大小限制
* 默认导出目录
* 是否恢复上次会话

---

# 24. 日志与错误处理

## 24.1 错误处理库

建议：

```toml
thiserror
anyhow
tracing
tracing-subscriber
```

使用规则：

* 库 crate 使用 `thiserror`。
* 应用层可以使用 `anyhow`。
* 日志使用 `tracing`。
* 用户界面显示简洁错误。
* 日志保存完整错误上下文。

## 24.2 禁止滥用 panic

正常运行路径禁止使用：

```rust
unwrap()
expect()
panic!()
unreachable!()
```

以下场景必须正常返回错误：

* 文件无法打开
* mmap 创建失败
* 文件被删除
* 磁盘空间不足
* FASTQ 截断
* 无效 UTF-8 Header
* 索引取消
* 搜索取消
* 保存取消
* 临时文件创建失败
* 路径包含非 ASCII 字符

## 24.3 错误信息示例

```text
FASTQ 记录结构错误

文件：reads.fastq
记录：12,845
文件偏移：1,492,382,102
问题：Sequence 长度为 151，Quality 长度为 149
```

---

# 25. 性能目标

参考环境：

* Windows 11
* x86-64
* 16 GiB RAM
* NVMe SSD
* Release 构建
* MSVC 工具链

| 场景                 |               目标 |
| ------------------ | ---------------: |
| 应用冷启动              |         小于 1.5 秒 |
| 100 MiB FASTA 首屏显示 |           小于 1 秒 |
| 1 GiB FASTA 首屏显示   |           小于 2 秒 |
| 4 GiB FASTA 首屏显示   |           小于 3 秒 |
| 1 GiB 文件查看内存       |       小于 300 MiB |
| 4 GiB 文件查看内存       |       小于 500 MiB |
| 已索引记录跳转            |        小于 100 ms |
| 滚动输入延迟 P95         |         小于 50 ms |
| 后台索引时界面帧率          |       不低于 30 FPS |
| 搜索首个结果             |         尽量小于 1 秒 |
| 流式导出吞吐             | 磁盘顺序写入速度的 60% 以上 |

性能结果必须记录：

* CPU 型号
* 内存容量
* 磁盘类型
* Windows 版本
* 文件大小
* 文件格式
* 记录数量
* 平均序列长度
* 平均物理行长度
* Debug 或 Release

不得伪造或估算性能测试结果。

---

# 26. 测试策略

## 26.1 单元测试

必须覆盖：

* 文件格式检测
* BOM 检测
* LF 检测
* CRLF 检测
* 混合换行检测
* FASTA 记录边界
* FASTA 空记录
* FASTA Header 提取
* FASTA ID 提取
* FASTQ 状态机
* 多行 FASTQ
* FASTQ 截断
* Sequence/Quality 长度不匹配
* 无文件末尾换行
* 非法质量字符
* GC 统计
* N 统计
* reverse complement
* FASTQ reverse complement
* wrap
* unwrap
* Overlay 顺序
* 导出失败清理
* 文件偏移转换

## 26.2 集成测试

必须覆盖：

* 打开小型 FASTA
* 打开小型 FASTQ
* 打开未知格式
* 后台索引完成
* 取消索引
* 搜索并跳转
* 导出当前记录
* 删除记录后重建文件
* 修改记录后重建文件
* 文件外部变化检测
* 中文路径
* 长路径
* 只读目录错误

## 26.3 属性测试

使用 `proptest` 测试：

* 随机 FASTA
* 随机 FASTQ
* 随机换行
* 随机截断位置
* 随机 Header
* 随机 Sequence
* 随机 Quality
* 随机 Overlay 操作组合

要求：

* 不 panic
* 不越界
* 不死循环
* 不生成倒置字节范围
* 不错误修改源文件

## 26.4 模糊测试

使用 `cargo-fuzz`：

* FASTA 解析器
* FASTQ 状态机
* 格式检测器
* ID 提取器
* 换行扫描器
* 局部记录验证器

## 26.5 大文件测试数据

大型文件不得提交到 Git 仓库。

使用脚本生成：

```powershell
.\scripts\generate-large-fasta.ps1 -SizeGB 1
.\scripts\generate-large-fasta.ps1 -SizeGB 4
.\scripts\generate-large-fastq.ps1 -Records 10000000
```

默认生成路径不得位于 C 盘。

脚本必须允许显式指定输出目录：

```powershell
-OutputDirectory D:\SeqFlashTestData
```

---

# 27. Windows 专项要求

必须测试：

* Windows 10
* Windows 11
* NTFS
* 中文用户名
* 中文目录
* 空格路径
* 超长路径
* 只读文件
* 网络映射盘
* 可移动磁盘
* 文件被其他程序占用
* 125% DPI
* 150% DPI
* 200% DPI
* 多显示器
* 深色模式
* 浅色模式

Windows 发布产物：

```text
SeqFlash.exe
SeqFlash-portable-x86_64.zip
SeqFlash-setup-x86_64.exe
```

首个开发阶段只要求生成 `SeqFlash.exe`。

安装器属于后期里程碑。

---

# 28. 安全规则

1. 默认以只读方式打开源文件。
2. 默认不覆盖源文件。
3. 不将整个大文件读入内存。
4. 不将整个文件转换为 `String`。
5. UI 线程不扫描完整文件。
6. 全文件任务必须可取消。
7. 文件偏移统一使用 `u64`。
8. `unsafe` 必须封装并说明安全前提。
9. 保存失败必须清理临时文件。
10. 文件外部变化时不得继续静默保存。
11. 无效格式不得导致崩溃。
12. 无效 UTF-8 不得导致文件无法打开。
13. 索引失败不得影响原始文本查看。
14. 后台任务不得直接修改 GUI 状态。
15. 旧 Revision 结果不得覆盖新 Revision。
16. 不记录用户完整序列内容到普通日志。
17. 日志不得默认包含敏感生物数据。
18. 崩溃报告中只记录文件路径和错误位置，不记录完整序列。

---

# 29. 代码质量要求

每个里程碑结束后执行：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

要求：

* 所有检查成功。
* 新增公共 API 有文档。
* 核心数据结构有测试。
* 不保留无用依赖。
* 不提交编译产物。
* 不提交大型测试文件。
* 不使用占位实现伪装功能完成。
* 不以注释代替验收功能。
* 不隐藏失败测试。
* 不随意禁用 Clippy lint。
* 不使用全局可变状态。
* 不在 UI 线程执行阻塞文件 I/O。

---

# 30. 开发里程碑

---

## M0：Windows 仓库初始化

### 目标

建立独立、可编译、可测试的 SeqFlash Rust workspace。

### 允许范围

* 初始化 Git 仓库
* 创建 Cargo workspace
* 创建基础 crate
* 配置 Rust MSVC
* 配置 `rustfmt`
* 配置 Clippy
* 配置 Windows GitHub Actions
* 创建最小 egui 主窗口
* 添加日志系统
* 添加错误处理框架
* 添加基础应用目录
* 添加 README
* 添加 LICENSE
* 添加开发文档

### 推荐初始 crate

```text
seqflash-app
seqflash-types
seqflash-settings
```

其他 crate 可以建立空目录和最小合法库，但不得实现业务功能。

### 禁止范围

* mmap
* FASTA 解析
* FASTQ 解析
* 文件索引
* 搜索
* 统计
* 编辑
* 导出
* SeqBrio 集成
* 插件系统
* 安装器

### 验收标准

以下命令全部成功：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

运行应用后：

* 显示 SeqFlash 主窗口。
* 标题为 `SeqFlash`。
* 窗口可以正常关闭。
* 不出现控制台 panic。
* 日志目录可以创建。
* 不要求任何 FASTA/FASTQ 功能。

---

## M1：文件打开和 mmap 文档模型

### 目标

能够安全打开本地大文件，并建立只读文档模型。

### 工作内容

* 文件打开对话框
* 文件拖放
* 读取文件元数据
* 只读 mmap
* 文档 ID
* 文档列表
* 文件关闭
* 多标签页基础
* 文件大小显示
* 文件路径显示
* 空文件处理
* 最近文件基础记录
* 中文路径支持
* 文件外部变化检测基础

### 禁止范围

* FASTA 完整索引
* FASTQ 完整索引
* 全文件搜索
* 序列统计
* 编辑
* 导出
* SeqBrio 集成

### 验收标准

* 可以打开 1 GiB 文件。
* 不将完整文件复制到 `Vec<u8>`。
* 内存占用不会接近文件大小。
* 可以关闭文件并释放 mmap。
* 打开失败不会创建损坏标签页。
* 支持中文路径。
* 支持无效 UTF-8 内容。
* 空文件不会崩溃。

---

## M2：虚拟原始文本查看器

### 目标

在不完整加载文件的情况下显示和滚动大型文本。

### 工作内容

* 可见区域计算
* 换行扫描
* 行检查点
* 原始文本绘制
* 鼠标滚轮
* Page Up/Page Down
* Home/End
* 跳转文件偏移
* 状态栏偏移显示
* 可见区域缓存
* 缓存淘汰
* 基础文本选择
* 复制可见文本

### 禁止范围

* Record ID 列表
* FASTQ 质量统计
* 记录编辑
* 全文件替换
* SeqBrio 集成

### 验收标准

* 1 GiB 文件首屏在目标时间内显示。
* 滚动时 UI 不冻结。
* 只绘制可见区域。
* 不为全部行创建 Widget。
* 可以跳转文件开头和结尾。
* 缓存不会持续无限增长。
* 无超长单行崩溃。
* 无文件末尾换行时可以显示最后一行。

---

## M3：FASTA 记录索引和导航

### 目标

实现大型 FASTA 文件的记录级浏览。

### 工作内容

* FASTA 格式检测
* 后台记录扫描
* `FastaRecordEntry`
* 索引进度
* 索引取消
* Record ID 提取
* 左侧记录列表
* 上一条/下一条
* 记录编号跳转
* 当前记录高亮
* 记录视图
* 当前记录长度
* GC%
* N 数量
* 非法字符检查

### 禁止范围

* FASTQ
* 全文件自由编辑
* SeqBrio 集成
* 磁盘索引缓存
* gzip

### 验收标准

* 1 GiB FASTA 可以边浏览边索引。
* 显示首屏不等待完整索引。
* 索引可取消。
* 记录偏移正确。
* ID 提取正确。
* 当前记录长度正确。
* GC 和 N 统计正确。
* 点击记录列表可以跳转。
* 非法字符可定位到文件偏移。

---

## M4：FASTQ 记录索引和验证

### 目标

实现 FASTQ 记录浏览和结构验证。

### 工作内容

* FASTQ 格式检测
* 状态机解析器
* 多行 Sequence
* 多行 Quality
* Sequence/Quality 长度匹配
* 截断检测
* 非法质量字符检查
* FASTQ 记录列表
* 当前记录显示
* 当前记录质量统计
* 错误记录面板
* 点击错误跳转

### 禁止范围

* BAM/CRAM
* 自动质量编码推断
* gzip 随机访问
* SeqBrio 集成

### 验收标准

* 标准四行 FASTQ 正确解析。
* 多行 FASTQ 正确解析。
* 截断 FASTQ 不导致崩溃。
* 长度不匹配可以报告。
* 错误包含记录编号和文件偏移。
* 错误列表可以跳转。
* 后台验证不阻塞 UI。
* 取消任务后状态正确。

---

## M5：搜索

### 目标

支持大型文件中的 ID 和序列搜索。

### 工作内容

* Record ID 精确搜索
* Record ID 前缀搜索
* 当前记录搜索
* 全文件字节搜索
* 序列片段搜索
* 搜索结果列表
* 上一个/下一个结果
* 搜索进度
* 任务取消
* 结果数量限制
* 搜索高亮

### 禁止范围

* 全功能正则表达式
* 模糊比对
* BLAST
* 全文件替换
* SeqBrio 集成

### 验收标准

* 搜索过程 UI 保持响应。
* 搜索可以取消。
* 首个结果可提前显示。
* 点击结果可以跳转。
* 结果包含字节偏移。
* 已索引时包含记录编号。
* 默认最多保存 10,000 条结果。
* 搜索不复制整个文件。

---

## M6：记录导出和序列操作

### 目标

支持安全的记录级操作。

### 工作内容

* 导出当前记录
* 导出多条选中记录
* 复制 Header
* 复制 Sequence
* 复制 Quality
* reverse complement
* FASTQ reverse complement
* 大小写转换
* FASTA wrap
* FASTA unwrap
* FASTQ 转 FASTA
* 按 ID 提取
* 按长度过滤

### 禁止范围

* 原地覆盖源文件
* 全文件自由编辑
* SeqBrio 集成
* 插件系统

### 验收标准

* 所有操作有单元测试。
* FASTQ reverse complement 同时反转 Quality。
* 导出为流式执行。
* 导出可取消。
* 导出失败清理临时文件。
* 源文件始终不变。
* 输出可以重新被 SeqFlash 打开。

---

## M7：记录级编辑和 Overlay

### 目标

实现有限、安全的记录级编辑。

### 工作内容

* 修改 Header
* 修改当前 Sequence
* 修改当前 Quality
* 删除当前记录
* 替换当前记录
* 插入记录
* `EditOverlay`
* 修改状态显示
* 撤销最近的记录级操作
* 重做最近的记录级操作
* 流式另存为
* 保存进度
* 保存取消
* 外部文件变化检查

### 禁止范围

* 任意文本自由编辑
* 超大记录直接完整编辑
* 原地保存
* SeqBrio 集成

### 验收标准

* 原始 mmap 始终只读。
* Overlay 不修改源文件。
* 删除和替换可以正确预览。
* 保存生成新文件。
* 保存失败不损坏源文件。
* 取消后清理临时文件。
* 超过编辑阈值的记录保持只读。
* Undo/Redo 不会破坏 Overlay 顺序。

---

## M8：稳定性和性能优化

### 目标

达到可供实际测试使用的 Beta 质量。

### 工作内容

* 性能基准
* 内存分析
* 长时间滚动测试
* 超长单行处理
* 超长单条序列处理
* 多文件标签测试
* 后台任务竞争测试
* Revision 过期结果测试
* mmap 生命周期测试
* 模糊测试
* 崩溃日志
* 错误提示优化

### 验收标准

* 达到主要性能目标。
* 连续运行两小时无明显内存增长。
* 频繁打开关闭文件无句柄泄漏。
* 快速切换标签不会串用任务结果。
* 取消任务不会死锁。
* 格式错误不会导致 panic。
* 关键解析器通过模糊测试。
* Release 构建无 Clippy 警告。

---

## M9：Windows 产品化

### 目标

生成普通 Windows 用户可以运行的版本。

### 工作内容

* 应用图标
* 版本信息
* 便携版 ZIP
* Windows 安装器
* 文件关联
* 最近文件
* 会话恢复
* DPI 优化
* 中文路径回归测试
* 长路径回归测试
* GitHub Release 工作流
* 用户使用文档

文件关联：

```text
.fa
.fasta
.fna
.ffn
.faa
.frn
.fq
.fastq
```

### 验收标准

* Windows 10/11 可运行。
* 用户无需安装 Rust。
* 便携版可独立运行。
* 安装版可创建文件关联。
* 双击 FASTA/FASTQ 可以打开。
* 125%、150%、200% DPI 可用。
* 中文用户名和路径可用。
* 发布包不包含开发测试数据。

---

# 31. SeqFlash 1.0 之后的候选功能

以下功能不属于当前开发范围：

## 31.1 索引缓存

```text
sample.fasta.seqflash-index
```

缓存可包含：

* 文件大小
* 修改时间
* 文件头尾哈希
* 索引版本
* 文件格式
* 记录边界
* ID 索引

## 31.2 gzip 支持

候选方式：

* 流式预览
* 后台解压到临时文件
* 提示磁盘空间
* BGZF 随机访问

普通 gzip 不承诺直接随机浏览。

## 31.3 更多格式

候选：

* GenBank
* GFF3
* BED
* VCF
* SAM

FASTA/FASTQ 体验稳定前不得实现。

## 31.4 SeqBrio 集成

未来可能：

* 通过 Rust API 集成
* 通过子进程调用
* 共享任务格式
* 从 SeqFlash 启动 SeqBrio 工作流

当前不得实现。

---

# 32. GLM 5.2 执行规则

GLM 5.2 在执行本计划时必须遵守：

1. 每次只执行一个里程碑。
2. 严格遵守当前里程碑范围冻结。
3. 不提前实现后续功能。
4. 修改前先阅读整个 workspace。
5. 不删除已有正常功能。
6. 不擅自更换 GUI 框架。
7. 不引入 SeqBrio 依赖。
8. 不创建 SeqBrio 占位接口。
9. 不添加未被当前里程碑使用的大型依赖。
10. 不将大型测试文件放在 C 盘。
11. 不将大型测试文件提交到 Git。
12. 不使用占位代码伪装完成。
13. 不使用假的性能测试结果。
14. 不隐藏失败测试。
15. 不跳过 Clippy。
16. 不通过大量 `allow` 消除警告。
17. 不在核心运行路径使用 `unwrap`。
18. 不扩大产品范围。
19. 每个里程碑结束后执行完整检查。
20. 每次报告新增、修改和删除的文件。
21. 每次报告验收标准结果。
22. 每次报告已知问题。
23. 编译失败时优先修复当前问题。
24. 遇到环境问题时不得转向实现后续功能。
25. 未完成验收项必须明确标记为未完成。

---

# 33. 每个里程碑的输出格式

GLM 5.2 完成任务后必须按照以下格式报告：

```text
# 里程碑结果

## 1. 完成范围

## 2. 新增文件

## 3. 修改文件

## 4. 删除文件

## 5. 主要实现

## 6. 测试命令和结果

- cargo fmt
- cargo clippy
- cargo test
- cargo build --release

## 7. 验收标准

- [x] 已通过
- [ ] 未通过

## 8. 性能结果

仅在实际运行性能测试后填写。

## 9. 已知问题

## 10. 下一里程碑建议

不得自动开始下一里程碑。
```

---

# 34. M0 首次执行指令

将下面的指令交给 GLM 5.2：

```text
SeqFlash M0 — Windows 仓库初始化

请阅读仓库根目录的 DEVELOPMENT_PLAN.md，并严格执行其中的 M0。

项目要求：

1. SeqFlash 是完全独立的 Rust Windows GUI 项目。
2. 不得依赖、调用或复制任何 SeqBrio 组件。
3. 使用 stable-x86_64-pc-windows-msvc。
4. GUI 固定使用 eframe + egui。
5. 当前只执行 M0，不得实现 M1 及后续功能。

M0 允许范围：

- 初始化 Cargo workspace
- 创建 seqflash-app
- 创建 seqflash-types
- 创建 seqflash-settings
- 创建其他 crate 的最小合法骨架
- 配置 rustfmt
- 配置 Clippy
- 配置 Windows GitHub Actions
- 添加 tracing 日志
- 添加 thiserror/anyhow 错误处理基础
- 创建最小 SeqFlash egui 主窗口
- 添加 README、LICENSE 和基础目录

M0 禁止范围：

- 文件 mmap
- FASTA/FASTQ 检测或解析
- 文件索引
- 搜索
- 序列统计
- 编辑
- 导出
- gzip
- 安装器
- SeqBrio 集成
- 插件系统

完成后运行：

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release

最终按照 DEVELOPMENT_PLAN.md 第 33 节的格式报告。

不得自动开始 M1。
```

---

# 35. 最终范围冻结

SeqFlash 1.0 的开发主线固定为：

```text
Windows GUI
    ↓
只读大文件打开
    ↓
虚拟原始文本查看
    ↓
FASTA 记录索引
    ↓
FASTQ 记录索引
    ↓
搜索和导航
    ↓
记录导出
    ↓
记录级 Overlay 编辑
    ↓
性能和稳定性
    ↓
Windows 发布
```

在 M0 至 M9 完成前，禁止将项目扩展为：

* 通用文本编辑器
* 完整生物信息学平台
* 跨平台应用
* 插件生态
* SeqBrio 图形前端
* 多格式组学浏览器

SeqFlash 当前唯一目标是：

> 在 Windows 上稳定、快速、低内存地打开和浏览大型 FASTA/FASTQ 文件，并提供安全、实用的记录级操作。
