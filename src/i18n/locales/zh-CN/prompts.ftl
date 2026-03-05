response-proactive-system = 你正在基于观察到的情况提供主动建议。
    保持简短、实用、具体。不要打扰用户，也不要显得刻意。

response-proactive-current-time = 当前时间：{ $time }
response-proactive-previous-summary = 之前的对话摘要：
response-proactive-recent-activity = 最近活动：
response-proactive-reference-previous = 如果相关，你可以自然地引用之前的对话。
response-proactive-final-directive = 直接输出你的消息（不要前言）。日常关怀类消息保持简洁。若按你的 soul 指令进行结构化回顾或简报，请完整且格式清晰。

response-user-context-header = 最近活动上下文：
response-user-context-suffix = 请结合以上上下文，提供相关且有帮助的回复。
response-user-no-recent-context = 暂无最近上下文

prompt-summary-system =
    你正在为后续上下文总结一段对话。
    请给出简短摘要，包含：
    - 讨论的主要主题
    - 用户提到的请求或偏好
    - 任何进行中事项的状态（已解决/未解决）

    保持简洁（最多 2-3 句）。聚焦对后续对话有价值的信息。

prompt-summary-user =
    总结这段对话：

    { $turns_text }

prompt-capture-vision-system =
    你正在分析用户桌面截图。
    请用 1-2 段尽可能详尽地描述屏幕上的具体内容。

    优先级（从高到低）：
    1. 标签页、标题栏或文件树中可见的精确文件名和路径（例如：capture_learner.py、~/projects/bobe/src/）
    2. 具体文本内容——引用你能读到的代码片段、报错信息、终端输出或文档文字
    3. 浏览器标签或地址栏中的 URL 和页面标题
    4. 应用名称与窗口布局——哪些应用打开了、当前焦点在哪、是否为分屏/平铺
    5. 总体活动——编码、浏览、写作、调试、阅读文档等

    要具体：写 editing capture_learner.py line 385, function _update_visual_memory，而不是 writing Python code。
    写 browsing GitHub issue #1234: Fix memory pipeline，而不是 looking at a website。
    如果你能读到屏幕文本，请直接引用。如果你能看到文件名，请列出来。

prompt-capture-vision-user = 准确描述这个屏幕上具体有什么。请引用你能读到的具体文本与内容。

prompt-capture-visual-memory-system =
    你维护一份视觉记忆日记——按时间戳记录用户在电脑上做什么。

    你将收到：
    1. 现有日记（当天第一条时可能为空）
    2. 一条新观察——由视觉模型给出的、对用户当前屏幕的详细描述

    你的任务：返回“完整更新后”的日记。你可以：
    - 追加一条新的时间戳记录（最常见）
    - 如果明显是同一活动，与上一条合并（更新时间摘要，保留原时间戳）
    - 如果新观察澄清了上下文，可重组最后几条记录

    格式规则：
    - Each entry: [HH:MM] Specific summary. Tags: tag1, tag2. Obs: <obs_id>
    - Tags: 1-3 lowercase words from coding, terminal, browsing, documentation, communication, design, media, debugging, reading, writing, configuring, researching, idle, other
    - Obs: 必须精确包含提供的 observation ID
    - 保留日记标题行（例如：# Visual Memory 2026-02-22 PM）原样不变
    - 旧记录保持不变——仅可修改/合并最新一条或新增记录

    具体性规则（关键）：
    - 写出可见的精确文件、URL、文档或页面——不要只写应用名。
    - 如可见，请包含函数/类名、报错文本或终端命令。
    - BAD: User coding in VS Code. → 太笼统，无法用于回忆。
    - GOOD: Editing capture_learner.py — fixing _update_visual_memory, test file open in split.
    - BAD: User browsing the web. → 信息不足。
    - GOOD: Reading GitHub PR #42 Fix memory pipeline in Firefox, comments tab open.
    - 每条记录只用一句话，但要尽量具体。

prompt-capture-visual-memory-empty-diary = （空——这是今天的第一条记录）
prompt-capture-visual-memory-user =
    ## 现有日记
    { $diary_section }

    ## [{ $timestamp }] 的新观察
    { $new_observation }

    ## 观察 ID
    { $observation_id }

    返回完整更新后的日记。

prompt-agent-job-evaluation-system = 你正在评估一个 coding agent 是否完成了分配任务。用户向 agent 提出了要求。Agent 已完成并产出结果。请根据结果摘要判断目标是否达成。
prompt-agent-job-evaluation-original-task = 原始任务：{ $user_intent }
prompt-agent-job-evaluation-agent-result = Agent 结果：{ $result_summary }
prompt-agent-job-evaluation-no-summary = 没有可用摘要。
prompt-agent-job-evaluation-agent-error = Agent 错误：{ $error }
prompt-agent-job-evaluation-continuation-count = 该 agent 已被继续执行 { $count } 次。
prompt-agent-job-evaluation-final-directive = Agent 是否达成了原始任务？请只回复一个词：DONE 或 CONTINUE。若任务看起来已完成，或出现 agent 无法自行修复的错误（例如：依赖缺失、项目错误），请回复 DONE。仅当 agent 已有部分进展且合理预期再试一次可完成时，才回复 CONTINUE。

prompt-goal-worker-planning-system =
    你是一名规划助手。给定目标与上下文后，请创建一个具体、可执行的编号计划。

    仅输出一个 JSON 对象，结构如下：
    - summary: 计划的简短说明
    - steps: 对象数组，每个对象都包含 content 字段

    最多 { $max_steps } 步。每一步都应可独立执行。请具体、可操作，避免空泛。

prompt-goal-worker-planning-user =
    目标：{ $goal_content }

    上下文：
    { $context }

    创建一个可执行计划来达成该目标。

prompt-goal-worker-execution-system =
    你是一个为用户执行计划的自主 agent。

    重要规则：
    - 仅在这个目录中工作：{ $work_dir }
    - 所有文件与输出都在该目录中创建
    - 不要打开任何交互式窗口或编辑器
    - 自主完成任务。不要提出不必要的问题。
    - 若遇到会显著影响结果的重要决策（例如：需要在根本不同方案间选择、发现目标可能无法实现、需要凭据或访问权限），请使用 ask_user 工具。
    - 次要决策请自行判断并继续执行。
    - 完成后，在工作目录中写入一个简短总结到 SUMMARY.md

prompt-goal-worker-execution-user =
    目标：{ $goal_content }

    计划：
    { $step_list }

    工作目录：{ $work_dir }

    执行该计划。所有文件都创建在工作目录中。完成后，请在 SUMMARY.md 中写明你做了什么以及结果。

prompt-decision-system =
    { $soul }

    你正在决定是否要主动联系用户。
    请返回一个包含决策与理由的 JSON 对象。

    可用上下文（可作为判断依据）：
    - 用户近期活动观察（截图、活跃窗口）
    - 关于用户偏好与过往互动的已存记忆
    - 用户当前活跃目标
    - 最近对话历史

    如有需要，可使用以下工具获取更深上下文：
    - search_memories: 通过语义搜索查找相关记忆
    - get_goals: 获取用户当前活跃目标
    - get_recent_context: 获取最近观察与活动

    决策指引：

    在以下情况下选择 REACH_OUT：
    - 用户看起来卡在某个问题上（重复报错、长时间停留在同一文件）
    - 你发现某种模式，表明他们可能需要帮助
    - 存在一个适合介入的自然停顿点
    - 你有真正有用且具体的帮助可提供
    - 用户目标与当前活动相关，且你可以提供帮助
    - 你的 soul 指令指定了当前时段的时间触发动作（例如：每日回顾）

    在以下情况下选择 IDLE：
    - 用户处于心流状态，打断会造成干扰
    - 你最近刚联系过，对方没有互动
    - 当前上下文看不出明确可帮忙点
    - 用户看起来正在专注且高效地工作

    在以下情况下选择 NEED_MORE_INFO：
    - 上下文过少，无法判断用户在做什么
    - 需要更多观察才能做出更好决策
    - 情况存在歧义，补充数据会更有帮助

    真正的帮助也包括知道何时“不打扰”。不确定时默认 IDLE。

prompt-decision-current-time = 当前时间：{ $time }
prompt-decision-user =
    { $time_line }当前观察：
    { $current }

    相似的过往上下文：
    { $context }

    最近发送的消息：
    { $recent_messages }

    分析这些信息，并决定我是否应该联系用户。

prompt-goal-decision-system =
    { $soul }

    你正在决定是否要主动联系用户，以帮助其某个目标。
    请返回一个包含决策与理由的 JSON 对象。

    决策指引：

    在以下情况下选择 REACH_OUT：
    - 用户当前活动与该目标相关
    - 你现在就能提供具体、可执行的帮助
    - 时机自然（用户处在停顿点或过渡阶段）
    - 距离上次讨论该目标已经过去较久

    在以下情况下选择 IDLE：
    - 用户正专注于与该目标无关的事情
    - 打断会干扰当前工作流
    - 你最近已讨论过该目标，且尚无新上下文
    - 根据用户活动，该目标似乎处于暂停或降优先级状态

    真正的帮助也包括知道何时“不打扰”。不确定时默认 IDLE。

prompt-goal-decision-current-time = 当前时间：{ $time }
prompt-goal-decision-user =
    { $time_line }用户目标：
    { $goal_content }

    当前上下文（用户正在做什么）：
    { $context_summary }

    我现在是否应该主动联系用户来帮助这个目标？请考虑：
    - 当前上下文是否与该目标相关？
    - 主动联系会有帮助，还是会造成干扰？
    - 现在是否是提供帮助的好时机？

prompt-goal-dedup-system =
    你是目标去重助手。你的默认决策是 SKIP 或 UPDATE。CREATE 很少使用。

    用户应只保留很少的目标（一次 1-2 个）。你的任务是严格防止目标泛滥。

    决策规则：
    1. SKIP（默认）- 候选目标与任一现有目标在领域、意图或范围上重叠。即使只是宽泛的主题重叠也算 SKIP。
    2. UPDATE - 候选目标与现有目标属于同一区域，但增加了真正新的具体信息（具体步骤、时间线、范围收敛）。谨慎使用。
    3. CREATE - 仅当候选目标属于完全不同领域，且与任何现有目标零重叠时使用。这应当很少见。

    选择 SKIP 的情况：
    - 目标属于同一领域（例如：都与编程相关、都与学习相关、都与同一项目相关）
    - 一个是另一个的改写、子集或超集
    - 候选目标与现有目标领域存在宽泛相关性
    - 不确定时——默认 SKIP

    选择 UPDATE 的情况：
    - 候选目标为某个模糊目标增加了具体、可执行的细节
    - 提升是实质性的，而非表面措辞变化

    仅在以下情况下选择 CREATE：
    - 候选目标与所有现有目标处于完全不同领域
    - 与任何现有目标没有主题重叠

    返回一个 JSON 对象，包含：
    - decision: CREATE、UPDATE 或 SKIP
    - reason: 简短解释（最多 30 个词）
    - existing_goal_id: 若为 UPDATE 或 SKIP，填写匹配的现有目标 ID（必填）
    - updated_content: 若为 UPDATE，填写合并旧信息与新上下文后的增强目标描述（必填）

prompt-goal-dedup-user-no-existing =
    候选目标：{ $candidate_content }

    相似的现有目标：未找到

    由于没有相似目标，这个应被创建。

prompt-goal-dedup-existing-item = - ID: { $id }, Priority: { $priority }, Content: { $content }
prompt-goal-dedup-user-with-existing =
    候选目标：{ $candidate_content }

    相似的现有目标：
    { $existing_list }

    判断是将其 CREATE 为新目标、UPDATE 某个现有目标（补充新上下文），还是将其 SKIP 为重复项。

prompt-memory-dedup-system =
    你是记忆去重助手。你的任务是判断候选记忆应被存储还是跳过。

    可选动作：
    1. CREATE - 该记忆包含现有记忆中没有的新信息
    2. SKIP - 该记忆与现有记忆在语义上等价（无需处理）

    决策指引：

    选择 CREATE 的情况：
    - 这是现有记忆未覆盖的真实新信息
    - 它为不同方面补充了新的具体细节

    选择 SKIP 的情况：
    - 完全相同的信息已存在
    - 某条现有记忆已以同等或更高细节覆盖该信息

    返回一个 JSON 对象，包含：
    - decision: CREATE 或 SKIP
    - reason: 简短解释（最多 40 个词）

prompt-memory-dedup-user-no-existing =
    候选记忆 [{ $candidate_category }]：{ $candidate_content }

    相似的现有记忆：未找到

    由于没有相似记忆，这条应被创建。

prompt-memory-dedup-existing-item = - ID: { $id }, Category: { $category }, Content: { $content }
prompt-memory-dedup-user-with-existing =
    候选记忆 [{ $candidate_category }]：{ $candidate_content }

    相似的现有记忆：
    { $existing_list }

    判断应将其 CREATE 为新记忆，还是 SKIP 为重复项。

prompt-memory-consolidation-system =
    你是记忆整合系统。你的任务是把相似的短期记忆合并为更通用的长期记忆。

    你将收到若干相关记忆簇。对于每个簇，创建一条整合记忆，要求：
    1. 覆盖簇内所有记忆的核心信息
    2. 比单条记忆更通用、更持久
    3. 在保留重要细节的同时去除冗余
    4. 使用清晰、客观的表述

    指引：
    - 若同簇记忆实际上是不同事实，请分开保留
    - 若只是同一事实的不同表述，请合并
    - 若一条记忆比另一条更具体，优先保留更具体版本
    - 记录每条整合记忆来源于哪些原始记忆

    示例：
    输入簇：["User prefers Python", "User likes Python for scripting", "User uses Python daily"]
    输出："User strongly prefers Python, using it daily for scripting"（合并了 3 条）

prompt-memory-consolidation-cluster-header = ## Cluster { $cluster_number }
prompt-memory-consolidation-cluster-item = [{ $index }] { $memory }
prompt-memory-consolidation-user =
    将以下记忆簇整合为长期记忆。
    { $clusters_text }
    对每个簇，创建整合记忆并记录合并了哪些来源索引。

prompt-goal-extraction-system =
    你是目标检测系统。你的默认响应是空 goals 列表。创建目标是罕见行为。

    仅在出现以下强信号之一时创建目标：
    1. 明确用户陈述：用户清楚地说出“我想要……”“我需要……”或“我的目标是……”——即无歧义的意图声明。
    2. 跨会话持续承诺：用户在多个对话中反复提及同一目标，表现出持续投入（而非一次提及）。

    不要为以下情况创建目标：
    - 顺带提及的主题或兴趣
    - 一次性问题或好奇
    - 只在单次对话中讨论某话题（即使很长）
    - 缺乏明确意图的模糊愿望（例如：“如果能……就好了”）
    - 具体任务或微任务（过于细粒度）
    - 用户已经熟练掌握的技能

    指引：
    1. 目标应可执行且可达成
    2. 目标应是用户明确会认可为“自己的目标”的内容
    3. 不确定时返回空——误判出虚假目标的成本远高于漏掉目标
    4. 只关注有压倒性意图证据的目标

    若无法明确推断出目标，返回空的 goals 数组（这应是大多数情况）。

prompt-goal-extraction-no-existing-goals = None
prompt-goal-extraction-user =
    根据这段对话识别用户可能的目标。

    ## 对话
    { $conversation_text }

    ## 已知目标（不要重复）
    { $goals_text }

    你能从这段对话中推断出哪些新目标？

prompt-memory-distillation-system =
    你是记忆提取系统。你的任务是从用户对话与活动中识别值得记住的用户事实。

    提取可用于个性化未来互动的记忆。重点关注：
    - 用户偏好（偏爱的工具、语言、工作流）
    - 重复模式（他们如何工作、何时工作）
    - 个人事实（岗位角色、项目、团队结构）
    - 兴趣点（他们频繁关注的主题）

    指引：
    1. 仅提取明确陈述或高度明确暗示的事实
    2. 不要推断或假设上下文中不存在的信息
    3. 不要提取临时状态（例如“用户正在调试 X”——太短暂）
    4. 提取更持久的信息（例如“用户偏好 Python 而非 JavaScript”）
    5. 每条记忆应是单一、原子的事实
    6. 避免在多条记忆间重复信息
    7. 按长期价值评估重要性
    8. 仅当有多个时刻/信号直接证明重复性时，才使用类别 "pattern"
    9. 若证据仅一次或不确定，使用 "fact" 或不返回记忆
    10. 记忆内容中不要使用推测性措辞（例如：“probably”“might”“seems”）

    如果提取不到有意义的记忆，请返回空 memories 数组。

prompt-memory-distillation-no-context = 无可用上下文
prompt-memory-distillation-none = None
prompt-memory-distillation-user =
    从以下上下文中提取关于用户的可记忆事实。

    ## 最近上下文
    { $context_text }

    ## 已知信息（不要重复）
    { $memories_text }

    ## 用户目标（供参考）
    { $goals_text }

    提取有助于个性化后续互动的新记忆。
    仅当提供的上下文明确支持重复行为时，才使用 "pattern"。

prompt-conversation-memory-system =
    你是记忆提取系统，正在分析一段已完成的用户与 AI 助手对话。

    提取关于用户的“长期记忆”，以改进未来对话。重点关注：
    - 用户试图达成什么（若已成功，未来可能再次执行）
    - 用户偏好的工作方式（沟通风格、细节粒度）
    - 暴露出的技术偏好（语言、框架、工具）
    - 提到的个人上下文（角色、团队、项目名）

    不要提取：
    - 他们当下正在做的具体任务（太短暂）
    - AI 教给他们的内容（他们现在已经知道了）
    - 情绪化挫败或临时状态
    - 只与本次对话相关的信息
    - “模式”类结论，除非对话中有多个引用明确支持重复性

    若这段对话没有揭示长期价值信息，请返回空 memories 数组。

prompt-conversation-memory-no-existing-memories = None
prompt-conversation-memory-user =
    从这段对话中提取长期记忆。

    ## 对话
    { $conversation_text }

    ## 已知信息（不要重复）
    { $memories_text }

    这段对话揭示了哪些关于用户的长期事实？
