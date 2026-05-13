# Warp Desktop — English (source-of-truth locale)
# 本文件由多 agent 并行编辑,各自维护自己的 SECTION,key 以 surface 前缀隔离避免冲突。
# 加 key 时 ctrl-F 找到对应 SECTION 头追加;新 surface 在文件末尾加新 SECTION。
#
# 命名规范:kebab-case,前缀按 surface,例 settings-ai-title / drive-folder-rename-title
# 变量插值用 Fluent { $name } 语法,不要拼接

# =============================================================================
# SECTION: common (Owner: foundation)
# =============================================================================

app-name = Warp
app-tagline = 個人とチームのためのローカルファーストなターミナル

common-ok = OK
common-cancel = キャンセル
common-apply = 適用
common-save = 保存
common-delete = 削除
common-confirm = 確認
common-close = 閉じる
common-reset = リセット
common-back = 戻る
common-next = 次へ
common-yes = はい
common-no = いいえ
common-continue = 続行
common-approve = 承認
common-deny = 拒否
common-import = インポート
common-upgrade = アップグレード
common-default = デフォルト
common-editing = 編集中
common-viewing = 表示中
common-tooltip-enter-edit-mode = クリックして編集を開始
common-tooltip-exit-edit-mode = クリックして編集を終了
common-restored = 復元済み
common-continued = 続行済み
common-send-feedback = フィードバックを送信
common-something-went-wrong = 問題が発生しました
common-no-results-found = 結果が見つかりません。
common-edit = 編集
common-add = 追加
common-remove = 削除
common-rename = 名前変更
common-copy = コピー
common-paste = 貼り付け
common-search = 検索
common-view = 表示
common-loading = 読み込み中…
common-error = エラー
common-warning = 警告
common-info = 情報
common-success = 成功
common-all = すべて
common-none = なし
common-unknown = 不明
common-open = 開く
common-restore = 復元
common-duplicate = 複製
common-export = エクスポート
common-trash = ゴミ箱
common-copy-link = リンクをコピー
common-untitled = 無題
common-retry = 再試行
common-maximize = 最大化
common-discard = 破棄
common-undo = 元に戻す
common-commit = コミット
common-push = プッシュ
common-publish = 公開
common-create = 作成
common-configure = 構成
common-dismiss = 閉じる
common-manage = 管理
common-failed = 失敗
common-done = 完了
common-working = 処理中
common-cut = 切り取り
common-previous = 前へ
common-suggested = 推奨
common-copied-to-clipboard = クリップボードにコピーしました
common-new = 新規
common-no-results = 結果なし
common-learn-more = 詳細
common-skip = スキップ
common-get-warping = Warp を始める
common-try-again = もう一度試す
common-settings = 設定
common-premium = プレミアム
common-recommended = おすすめ
common-enabled = 有効
common-disabled = 無効
common-free = 無料
common-subscribe = 購読
common-list-prefix = {" - "}
common-current-directory = 現在のディレクトリ

# =============================================================================
# =============================================================================
# SECTION: agent-management (Owner: agent-i18n-remaining)
# Files: app/src/ai/agent_management/**
# =============================================================================

agent-management-filter-all-tooltip = 自分のエージェントタスクと共有チームタスクをすべて表示
agent-management-filter-personal = 個人
agent-management-filter-personal-tooltip = 自分が作成したエージェントタスクを表示
agent-management-get-started = はじめる
agent-management-view-agents = エージェントを表示
agent-management-clear-filters = フィルターをクリア
agent-management-clear-all = すべてクリア
agent-management-new-agent = 新規エージェント
agent-management-status = ステータス
agent-management-source = ソース
agent-management-created-on = 作成日
agent-management-has-artifact = アーティファクトあり
agent-management-harness = ハーネス
agent-management-environment = 環境
agent-management-created-by = 作成者
agent-management-last-24-hours = 過去 24 時間
agent-management-past-3-days = 過去 3 日間
agent-management-last-week = 先週
agent-management-artifact-pull-request = プルリクエスト
agent-management-artifact-plan = プラン
agent-management-artifact-screenshot = スクリーンショット
agent-management-artifact-file = ファイル
agent-management-source-scheduled = スケジュール
agent-management-source-local-agent = Warp (ローカルエージェント)
agent-management-source-cloud-agent = Warp (ローカルエージェント)
agent-management-source-oz-web = Oz Web
agent-management-source-github-action = GitHub Action
agent-management-no-session-available = 利用可能なセッションがありません
agent-management-session-expired = セッション期限切れ
agent-management-session-expired-tooltip = セッションは 1 週間で期限切れとなり、開けなくなります。
agent-management-metadata-source = ソース: { $source }
agent-management-metadata-harness = ハーネス: { $harness }
agent-management-metadata-run-time = 実行時間: { $run_time }
agent-management-metadata-credits-used = 使用クレジット: { $usage }
agent-management-environment-selected = 環境: { $environment }
agent-management-loading-cloud-runs = エージェントの実行を読み込み中

# =============================================================================
# SECTION: workspace-runtime (Owner: agent-i18n-remaining)
# Files: app/src/workspace/view.rs
# =============================================================================

workspace-menu-update-warp-manually = Warp を手動で更新
workspace-menu-whats-new = 新機能
workspace-menu-settings = 設定
workspace-menu-keyboard-shortcuts = キーボードショートカット
workspace-menu-documentation = ドキュメント
workspace-menu-feedback = フィードバック
workspace-menu-view-warp-logs = Warp ログを表示
workspace-menu-slack = Slack
workspace-toast-failed-load-conversation = 会話の読み込みに失敗しました。
workspace-toast-failed-load-conversation-for-forking = フォーク用の会話の読み込みに失敗しました。
workspace-toast-conversation-forking-failed = 会話のフォークに失敗しました。
workspace-toast-no-terminal-pane-for-context = ターミナルペインが開いていません。コンテキストとして添付するには新しいペインを開いてください。
workspace-toast-plan-already-in-context = このプランは既にコンテキストに含まれています。
workspace-toast-command-still-running = このセッションでコマンドがまだ実行中です。
workspace-toast-cannot-open-terminal-session = 新しいターミナルセッションを開けません
workspace-toast-out-of-ai-credits = AI クレジットが不足しているようです。
workspace-toast-upgrade-more-credits = アップグレードしてクレジットを追加。
workspace-toast-disabled-synchronized-inputs = すべての同期入力を無効化しました。
workspace-toast-conversation-deleted = 会話を削除しました
workspace-search-repos-placeholder = リポジトリを検索
workspace-search-tabs-placeholder = タブを検索...
workspace-rearrange-toolbar-items = ツールバー項目を並べ替え
workspace-new-session-agent = エージェント
workspace-new-session-terminal = ターミナル
workspace-new-session-cloud-oz = Ambient Agent
workspace-new-session-local-docker-sandbox = ローカル Docker サンドボックス
workspace-new-worktree-config = 新規 worktree 設定
workspace-new-tab-config = 新規タブ設定
workspace-reopen-closed-session = 閉じたセッションを再度開く
workspace-update-and-relaunch-warp = Warp を更新して再起動
workspace-updating-to-version = ({ $version }) に更新中
workspace-update-warp-manually = Warp を手動で更新
pane-get-started-title = はじめる
pane-new-tab-title = 新規タブ
# =============================================================================
# SECTION: terminal-runtime (Owner: agent-i18n-remaining)
# Files: app/src/terminal/view.rs
# =============================================================================

terminal-banner-completions-not-working-prefix = 補完が機能していないようです (
terminal-banner-more-info-lower = 詳細
terminal-banner-more-info = 詳細
terminal-banner-completions-not-working-middle = )。{" "}
terminal-banner-settings = 設定
terminal-banner-completions-not-working-suffix =  で tmux warpification を有効にすると解決する場合があります。
terminal-banner-shell-config-incompatible = シェルの設定が Warp と互換性がありません...{"  "}
terminal-banner-did-you-intend = もしかして {" "}
terminal-banner-move-cursor =  でカーソルを移動しようとしましたか?
terminal-toast-powershell-subshells-not-supported = PowerShell サブシェルは未対応
terminal-dont-ask-again = 次回から確認しない
terminal-clear-upload = アップロードをクリア
terminal-manage-defaults = デフォルトを管理
terminal-free-credits = 無料クレジット
terminal-cloud-agent-run = エージェント実行
terminal-agent-header-for-terminal = ターミナル用
ai-document-show-version-history = 履歴を表示
ai-document-update-agent = エージェントを更新
ai-document-save-and-sync-tooltip = この計画を Warp Drive に保存して自動同期
ai-document-show-in-warp-drive = Warp Drive で表示
ai-document-save-as-markdown-file = Markdown ファイルとして保存
ai-document-attach-to-active-session = アクティブセッションに添付
ai-document-copy-plan-id = プランIDをコピー
ai-document-plan-id-copied = プランIDをクリップボードにコピーしました
ai-conversation-view-in-oz = Oz で表示
ai-conversation-view-in-oz-tooltip = この実行を Oz Web アプリで表示
ai-artifact-prepare-download-failed = ファイルダウンロードの準備に失敗しました。
ai-block-open-in-github = GitHub で開く
ai-block-open-in-code-review = コードレビューで開く
ai-block-manage-rules = ルールを管理
ai-block-review-changes = 変更をレビュー
ai-block-open-all-in-code-review = すべてコードレビューで開く
ai-block-dont-show-again = 次回から表示しない
ai-block-rewind = 巻き戻し
ai-block-rewind-tooltip = このブロックの直前まで巻き戻す
ai-block-remove-queued-prompt = キュー中のプロンプトを削除
ai-block-send-now = 今すぐ送信
ai-block-check-now =  · 今すぐ確認
ai-block-check-now-tooltip = タイマーをスキップしてエージェントに今このコマンドを確認させる。
ai-block-resume-conversation = 会話を再開
ai-block-continue-conversation = 会話を続ける
ai-block-fork-conversation = 会話をフォーク
ai-block-show-credit-usage-details = クレジット使用詳細を表示
ai-block-follow-up-existing-conversation = 既存の会話でフォローアップ
ai-block-accept = 承認
ai-block-auto-approve = 自動承認
ai-rule-add-rule = ルールを追加
ai-rule-edit-rule = ルールを編集
ai-rule-delete-rule = ルールを削除
ai-aws-refresh-credentials = AWS 認証情報を更新
ai-footer-enable-notifications = 通知を有効化
ai-footer-enable-notifications-tooltip = Warp プラグインをインストールして Warp 内のリッチなエージェント通知を有効化
ai-footer-notifications-setup-instructions = 通知セットアップ手順
ai-footer-install-plugin-instructions-tooltip = Warp プラグインのインストール手順を表示
ai-footer-update-warp-plugin = Warp プラグインを更新
ai-footer-plugin-update-available-tooltip = Warp プラグインの新バージョンがあります
ai-footer-plugin-update-instructions = プラグイン更新手順
ai-footer-plugin-update-instructions-tooltip = Warp プラグインの更新手順を表示
ai-footer-context-window-usage-tooltip = コンテキストウィンドウ使用量
ai-footer-choose-environment-tooltip = 環境を選択
ai-footer-reasoning-depth-tooltip = 推論の深さ
ai-footer-file-explorer = ファイルエクスプローラー
ai-footer-open-file-explorer = ファイルエクスプローラーを開く
ai-footer-rich-input = リッチ入力
ai-footer-open-rich-input = リッチ入力を開く
ai-footer-open-coding-agent-settings = コーディングエージェント設定を開く
ai-ask-user-question-placeholder = 回答を入力して Enter キーを押す
ai-ask-user-questions-skipped = 質問をスキップ
ai-ask-user-answered-question = 質問に回答済み
ai-ask-user-answered-all-questions = { $total } 個の質問すべてに回答済み
ai-ask-user-answered-count = { $total } 個中 { $answered_count } 個の質問に回答済み
ai-code-diff-requested-edit-title = リクエストされた編集
ai-cloud-setup-visit-oz = Oz を開く
ai-inline-code-diff-review-changes = 変更をレビュー
ai-execution-profile-name-placeholder = 例: "YOLO code"
ai-execution-profile-delete-profile = プロファイルを削除
ai-notifications-mark-all-as-read = すべて既読にする
ai-assistant-copy-transcript-tooltip = 文字起こしをクリップボードにコピー
code-comment = コメント
code-copy-file-path = ファイルパスをコピー
code-select-all = すべて選択
code-replace-all = すべて置換
code-goto-line-placeholder = 行番号:列
code-open-file-unavailable-remote-tooltip = リモートセッションではファイルを開けません
code-view-markdown-preview = Markdown プレビューを表示
code-review-commit-and-create-pr = コミットして PR を作成
notebook-link-text-placeholder = テキスト
notebook-link-url-placeholder = リンク (Web またはファイル)
notebook-block-embed = 埋め込み
notebook-block-divider = 区切り線
notebook-insert-block-tooltip = ブロックを挿入
notebook-refresh-notebook = ノートブックを更新
notebook-refresh-file = ファイルを更新
notebook-open-in-editor = エディターで開く
notebook-sign-in-to-edit = 編集にはサインイン
editor-custom-keybinding = カスタム...
editor-change-keybinding = キーバインドを変更
autosuggestion-ignore-this-suggestion = この提案を無視
codex-use-latest-model = 最新の codex モデルを使用
openwarp-launch-visit-repo = リポジトリを見る
openwarp-launch-title = Warp がオープンソースになりました
openwarp-launch-description = コミュニティの皆さんがエージェントファーストのワークフローで Warp の構築に参加できます。
openwarp-launch-contribute-title = コントリビュート
openwarp-launch-contribute-description = Warp のクライアントコードがオープンソースになりました。/feedback スキルで Issue を作成し、こちらのコントリビューションガイドラインに従ってください。
openwarp-launch-contribute-link-text = こちら
openwarp-launch-oad-title = Open Automated Development
openwarp-launch-oad-description = Warp リポジトリは、Oz のローカルエージェント体験を活用したエージェントファーストワークフローで管理されています。
openwarp-launch-auto-model-title = 'auto (open-weights)' 登場
openwarp-launch-auto-model-description = タスクに最適なオープンウェイトモデル (Kimi、MiniMax など) を選ぶ新しい auto モデルを追加しました。
hoa-see-whats-new = 新着情報を見る
hoa-finish = 完了
session-config-get-warping = Warping を開始
uri-custom-uri-invalid = カスタム URI が無効です。
context-node-install-nvm = nvm をインストール
context-node-install-node = nvm install node
context-node-installed = インストール済み
context-chip-change-git-branch = Git ブランチを変更
context-chip-view-pull-request = プルリクエストを表示
context-chip-change-working-directory = 作業ディレクトリを変更
context-chip-working-directory = 作業ディレクトリ
settings-ai-repo-placeholder = 例: ~/code-repos/repo
settings-ai-commands-comma-separated-placeholder = コマンド (カンマ区切り)
settings-ai-regex-example-placeholder = 例: ls .*
settings-ai-command-supports-regex-placeholder = コマンド (正規表現対応)
settings-ai-aws-login-placeholder = aws login
settings-ai-default-placeholder = デフォルト
settings-working-directory-path-placeholder = ディレクトリパス
settings-startup-shell-executable-path-placeholder = 実行ファイルパス
settings-agent-providers-base-url-placeholder = https://api.deepseek.com/v1
drive-sharing-only-people-invited = 招待された人のみ
drive-sharing-anyone-with-link = リンクを知っている全員
drive-sharing-only-invited-teammates = 招待されたチームメイトのみ
drive-sharing-teammates-with-link = リンクを知っているチームメイト
terminal-warpify-subshell = サブシェルを Warpify
terminal-warpify-subshell-tooltip = このセッションで Warp シェル統合を有効化
terminal-use-agent = エージェントを使う
terminal-use-agent-tooltip = Warp エージェントに支援を依頼
terminal-give-control-back-to-agent = 制御をエージェントに戻す
terminal-resume-agent-tooltip = Warp エージェントに再開を依頼
terminal-voice-input-tooltip = 音声入力
terminal-attach-file-tooltip = ファイルを添付
terminal-slash-commands-tooltip = スラッシュコマンド
terminal-manage-api-keys-tooltip = API キーを管理
terminal-profiles = プロファイル
terminal-manage-profiles = プロファイルを管理
terminal-continue-locally = ローカルで続行
terminal-fork-conversation-locally-tooltip = この会話をローカルでフォーク
terminal-open-in-warp = Warp で開く
terminal-open-conversation-in-warp-tooltip = この会話を Warp デスクトップアプリで開く
terminal-stop-sharing = 共有を停止
terminal-copy-session-sharing-link = セッション共有リンクをコピー
terminal-shared-session-make-editor = 編集者にする
terminal-shared-session-make-viewer = 閲覧者にする
terminal-shared-session-change-role = 役割を変更
terminal-choose-execution-profile-tooltip = AI 実行プロファイルを選択
terminal-choose-agent-model-tooltip = エージェントモデルを選択
terminal-input-cli-agent-rich-input-hint = 作りたいものをエージェントに伝える...
terminal-input-enter-prompt-for-agent = { $agent } へのプロンプトを入力...
terminal-input-cloud-agent-hint = エージェントを起動
terminal-input-a11y-label = コマンド入力。
terminal-input-a11y-helper = シェルコマンドを入力し、Enter で実行。Cmd+↑ で過去に実行したコマンドの出力に移動。Cmd+L でコマンド入力に再フォーカス。
terminal-input-ai-command-search-hint = '#' を入力して AI コマンド候補を表示
terminal-input-run-commands-hint = コマンドを実行
terminal-input-agent-hint-deploy-react-vercel = 何でも Warp で。例: React アプリを Vercel にデプロイし環境変数を設定
terminal-input-agent-hint-debug-python-ci = 何でも Warp で。例: CI で Python テストが失敗する原因をデバッグ
terminal-input-agent-hint-setup-microservice = 何でも Warp で。例: Docker で新しいマイクロサービスをセットアップしデプロイパイプラインを作成
terminal-input-agent-hint-fix-node-memory-leak = 何でも Warp で。例: Node.js アプリのメモリリークを発見して修正
terminal-input-agent-hint-backup-postgres = 何でも Warp で。例: PostgreSQL DB のバックアップスクリプトを作成しスケジュール
terminal-input-agent-hint-migrate-mysql-postgres = 何でも Warp で。例: MySQL から PostgreSQL へのデータ移行
terminal-input-agent-hint-monitor-aws = 何でも Warp で。例: AWS インフラの監視とアラートを設定
terminal-input-agent-hint-build-fastapi = 何でも Warp で。例: FastAPI でモバイルアプリ向け REST API を構築
terminal-input-agent-hint-optimize-sql = 何でも Warp で。例: 遅い SQL クエリを最適化
terminal-input-agent-hint-github-actions = 何でも Warp で。例: マージ時に自動デプロイする GitHub Actions ワークフローを作成
terminal-input-agent-hint-redis-cache = 何でも Warp で。例: Web アプリに Redis キャッシュを設定
terminal-input-agent-hint-kubernetes-pods = 何でも Warp で。例: Kubernetes Pod がクラッシュし続ける原因をトラブルシュート
terminal-input-agent-hint-bigquery-pipeline = 何でも Warp で。例: CSV を処理して BigQuery に投入するデータパイプラインを構築
terminal-input-agent-hint-ssl-https = 何でも Warp で。例: SSL 証明書を設定してドメインを HTTPS 化
terminal-input-agent-hint-refactor-legacy-code = 何でも Warp で。例: レガシーコードをモダンな設計パターンにリファクタ
terminal-input-agent-hint-unit-tests = 何でも Warp で。例: 認証サービスのユニットテストを作成
terminal-input-agent-hint-elk-logs = 何でも Warp で。例: 分散システム向けに ELK スタックでログ集約を構築
terminal-input-agent-hint-oauth-express = 何でも Warp で。例: Express.js アプリに OAuth2 認証を実装
terminal-input-agent-hint-optimize-docker = 何でも Warp で。例: ビルド時間とサイズを削減するため Docker イメージを最適化
terminal-input-agent-hint-ab-testing = 何でも Warp で。例: Web アプリ向けに A/B テスト基盤を構築
terminal-input-steer-agent-hint = 実行中のエージェントを誘導
terminal-input-steer-agent-backspace-hint = 実行中のエージェントを誘導、または Backspace で終了
terminal-input-follow-up-hint = フォローアップを質問
terminal-input-follow-up-backspace-hint = フォローアップを質問、または Backspace で終了
terminal-input-search-queries = クエリを検索
terminal-input-search-queries-rewind = 巻き戻し先のクエリを検索
terminal-input-search-conversations = 会話を検索
terminal-input-search-skills = スキルを検索
terminal-input-search-models = モデルを検索
terminal-input-search-profiles = プロファイルを検索
terminal-input-search-commands = コマンドを検索
terminal-input-search-prompts = プロンプトを検索
terminal-input-search-indexed-repos = インデックス済みリポジトリを検索
terminal-input-search-plans = プランを検索
terminal-input-choose-agent-model = エージェントモデルを選択
terminal-message-new-agent-conversation = {" "}新しい /agent 会話
terminal-message-agent-for-new-conversation = 新しい会話には /agent
terminal-message-selected-text-attached = 選択テキストをコンテキストとして添付
terminal-message-to-remove = {" "}で削除
terminal-message-to-dismiss = {" "}で閉じる
terminal-message-plan-with-agent = {" "}エージェントと計画
terminal-message-continue-conversation = {" "}で会話を続ける
terminal-message-to-execute = {" "}で実行
terminal-message-to-send = {" "}で送信
terminal-message-open-conversation-title = {" "}で '{ $title }' を開く
terminal-message-autodetected = {" "}(自動検出){" "}
terminal-message-to-override = {" "}で上書き
terminal-message-to-navigate = {" "}でナビゲート
terminal-message-to-cycle-tabs = {" "}でタブを切り替え
terminal-message-to-select = {" "}で選択
terminal-message-select-save-profile = {" "}選択してプロファイルに保存
terminal-message-open-plan = {" "}プランを開く
terminal-starting-shell = シェルを起動中...
terminal-input-no-skills-found = スキルが見つかりません
terminal-model-specs-title = モデル仕様
terminal-model-specs-description = Warp のハーネスでのモデル性能、クレジット消費レート、タスク速度のベンチマーク。
terminal-model-specs-reasoning-level-title = 推論レベル
terminal-model-specs-reasoning-level-description = 推論レベルを上げるとクレジット消費とレイテンシが増えますが、複雑なタスクでの性能が向上します。
terminal-model-auto-mode-title = Auto モード
terminal-model-auto-mode-description = Auto はタスクに最適なモデルを選択します。Cost-efficiency はコスト最適化、Responsiveness は応答速度を最適化します。
terminal-model-banner-base-agent = ベースエージェントを使用中。フルターミナル使用モデルはフルターミナル使用エージェント専用です。
terminal-model-banner-full-terminal-agent = フルターミナル使用エージェントを使用中。ベースモデルはベースエージェント専用です。
terminal-filter-block-output-placeholder = ブロック出力をフィルター
# =============================================================================
# SECTION: object-surfaces (Owner: agent-i18n-remaining)
# Files: app/src/code_review/**, app/src/notebooks/**, app/src/workflows/**, app/src/drive/**
# =============================================================================

code-review-tooltip-show-file-navigation = ファイルナビゲーションを表示
code-review-discard-changes = 変更を破棄
code-review-create-pr = PR を作成
code-review-add-diff-set-context = diff セットをコンテキストに追加
code-review-show-saved-comment = 保存済みコメントを表示
code-review-add-comment = コメントを追加
code-review-discard-all = すべて破棄
code-review-initialize-codebase = コードベースを初期化
code-review-initialize-codebase-tooltip = コードベースのインデックス化と WARP.md を有効化します
code-review-open-repository = リポジトリを開く
code-review-open-repository-tooltip = リポジトリに移動してコーディング用に初期化します
code-review-open-file = ファイルを開く
code-review-add-file-diff-context = ファイル diff をコンテキストに追加
code-review-copy-file-path = ファイルパスをコピー
code-review-no-open-changes = 開いている変更はありません
code-review-header-reviewing-changes = コード変更をレビュー中
code-review-search-diff-placeholder = 比較する diff セットまたはブランチを検索…
code-review-one-comment = コメント 1 件
code-review-copy-text = テキストをコピー
code-review-file-level-comment-cannot-edit = ファイルレベルのコメントは現在編集できません。
code-review-outdated-comment-cannot-edit = 古くなったコメントは編集できません。
code-review-view-in-github = GitHub で表示
notebook-menu-attach-active-session = アクティブセッションにアタッチ
object-menu-open-on-desktop = デスクトップで開く
notebook-tooltip-restore-from-trash = ノートブックをゴミ箱から復元
notebook-tooltip-copy-to-personal = ノートブックの内容を個人ワークスペースにコピー
notebook-copy-to-personal = 個人ワークスペースにコピー
notebook-tooltip-copy-to-clipboard = ノートブックの内容をクリップボードにコピー
notebook-copy-all = すべてコピー
object-toast-link-copied = リンクをクリップボードにコピーしました
drive-toast-finished-exporting = オブジェクトのエクスポートが完了しました

# =============================================================================
# SECTION: remaining-settings-tabs-env (Owner: agent-i18n-remaining)
# Files: app/src/settings_view/**, app/src/tab_configs/**, app/src/env_vars/**
# =============================================================================

settings-environment-delete-button = 環境を削除
settings-language-system-default = システム既定
settings-language-english = English
tab-config-open-tab = タブを開く
tab-config-make-default = 既定にする
tab-config-already-default = すでに既定です
tab-config-edit-config = 設定を編集
env-vars-restore-tooltip = 環境変数をゴミ箱から復元
env-vars-variables-label = 変数

# =============================================================================
# SECTION: onboarding-callout (Owner: agent-i18n-remaining)
# Files: crates/onboarding/src/callout/view.rs
# =============================================================================

onboarding-callout-meet-input-title = Warp 入力欄のご紹介
onboarding-callout-meet-input-text-prefix = ターミナル入力欄はターミナルコマンドとエージェントへのプロンプトの両方を受け付け、どちらを使っているか自動的に判定します。
onboarding-callout-meet-input-text-suffix = を使うと、入力欄をエージェントモード (自然言語) またはターミナルモード (コマンド) に固定できます。
onboarding-callout-talk-agent-title = エージェントと対話する
onboarding-callout-talk-agent-text = 自然言語で入力してエージェントと対話できます。下記のクエリを送信して始めましょう: このリポジトリにはどんなテストがあり、どのように構成され、何をカバーしていますか?
onboarding-callout-skip = スキップ
onboarding-callout-submit = 送信
onboarding-callout-finish = 完了
onboarding-callout-meet-terminal-title = ターミナル入力欄のご紹介
onboarding-callout-meet-updated-terminal-title = 更新されたターミナル入力欄のご紹介
onboarding-callout-meet-terminal-text-prefix = ターミナルからコマンドを実行するか、
onboarding-callout-meet-terminal-text-suffix = を使ってエージェントを起動・送信します。
onboarding-callout-nl-overrides-title = 自然言語の上書き
onboarding-callout-nl-overrides-text-prefix = 自動判定はいつでも次の方法で上書きできます:
onboarding-callout-nl-support-title = 自然言語サポート
onboarding-callout-nl-support-text-prefix = 自然言語入力は既定で無効です。有効にすると、平易な英語でリクエストを入力でき、Warp がエージェント向けクエリを自動判定します。次の方法でいつでも上書きできます:
onboarding-callout-enable-nl-detection = 自然言語判定を有効にする
onboarding-callout-new-agent-title = Warp の新しいエージェント体験のご紹介
onboarding-callout-new-agent-text = エージェントとの会話は、ターミナルとは別のスコープを持つ画面になりました。ESC を押せばいつでもターミナルに戻れます。
onboarding-callout-updated-agent-input-title = 更新されたエージェント入力欄
onboarding-callout-updated-agent-input-project-text = エージェント入力欄は既定で自然言語とコマンドの両方を判定します。! を使うと bash モードに固定してコマンドを書けます。\n\n下記のクエリを送信してエージェントにこのプロジェクトを初期化させるか、⊗ で入力をクリアして自分で始めましょう!
onboarding-callout-skip-initialization = 初期化をスキップ
onboarding-callout-initialize = 初期化
onboarding-callout-updated-agent-input-text = エージェント入力欄は既定で自然言語とコマンドの両方を判定します。! を使うと bash モードに固定してコマンドを書けます。
onboarding-callout-back-terminal = ターミナルに戻る

# =============================================================================
# SECTION: language (Owner: foundation)
# Files: app/src/settings_view/appearance_page.rs (Language widget + restart modal)
# =============================================================================

language-widget-label = 言語
language-widget-secondary = 変更を完全に反映するには Warp を再起動してください。
language-restart-required-title = 言語が変更されました
language-restart-required-body = Warp の UI 言語が更新されました。一部のテキストは即座に切り替わりますが、すべての箇所に反映するには再起動が必要です。
# =============================================================================
# SECTION: settings (Owner: agent-settings)
# Files: app/src/settings_view/**
# =============================================================================

# --- ANCHOR-SUB-MOD-NAV (agent-settings-mod) ---
# settings_view/mod.rs SettingsSection Display labels + context menu pane actions

# Sidebar / SettingsSection labels (Display impl)
settings-section-about = 概要
settings-section-account = アカウント
settings-section-mcp-servers = MCP サーバー
settings-section-billing-and-usage = 請求と使用状況
settings-section-appearance = 外観
settings-section-features = 機能
settings-section-keybindings = キーボードショートカット
settings-section-privacy = プライバシー
settings-section-referrals = 紹介
settings-section-shared-blocks = 共有ブロック
settings-section-warp-drive = Warp Drive
settings-section-warpify = Warpify
settings-section-ai = AI
settings-section-warp-agent = Warp エージェント
settings-section-agent-profiles = プロファイル
settings-section-agent-mcp-servers = MCP サーバー
settings-section-agent-providers = プロバイダー
settings-section-knowledge = ナレッジ
settings-section-third-party-cli-agents = サードパーティ CLI エージェント
settings-section-code = コード
settings-section-editor-and-code-review = エディタとコードレビュー
settings-section-cloud-environments = 環境
settings-section-oz-cloud-api-keys = Oz Cloud API キー
settings-title = 設定

# Context menu items (split / close pane)
settings-pane-split-right = ペインを右に分割
settings-pane-split-left = ペインを左に分割
settings-pane-split-down = ペインを下に分割
settings-pane-split-up = ペインを上に分割
settings-pane-close = ペインを閉じる

# Debug toggle setting descriptions (command palette)
settings-debug-show-init-block = 初期化ブロックを表示
settings-debug-hide-init-block = 初期化ブロックを非表示
settings-debug-show-inband-blocks = インバンドコマンドブロックを表示
settings-debug-hide-inband-blocks = インバンドコマンドブロックを非表示

# --- ANCHOR-SUB-ABOUT (agent-settings-about) ---
# 此锚点下放 settings_view/about_page.rs + main_page.rs 字符串
# 命名前缀:settings-about-* / settings-main-*

# about_page.rs
settings-about-copyright = Copyright 2026 Warp
settings-about-automatic-updates-label = 自動更新
settings-about-automatic-updates-description = 有効にすると、OpenWarp はバックグラウンドで新バージョンを確認しダウンロードします。無効でも手動で更新を確認できます。

# main_page.rs — referral / account
settings-main-referral-cta = 友人や同僚に Warp を紹介して特典を獲得
settings-main-refer-a-friend = 友人を紹介
settings-main-sign-up = サインアップ
settings-main-plan-free = Free
settings-main-compare-plans = プランを比較
settings-main-contact-support = サポートに問い合わせ
settings-main-manage-billing = 請求を管理
settings-main-upgrade-to-turbo = Turbo プランにアップグレード
settings-main-upgrade-to-lightspeed = Lightspeed プランにアップグレード

# main_page.rs — settings sync
settings-main-settings-sync-label = 設定の同期

# main_page.rs — version / autoupdate
settings-main-version-label = バージョン
settings-main-status-up-to-date = 最新です
settings-main-cta-check-for-updates = 更新を確認
settings-main-status-checking = 更新を確認中...
settings-main-status-downloading = 更新をダウンロード中...
settings-main-status-update-available = 更新があります
settings-main-cta-relaunch-warp = Warp を再起動
settings-main-status-updating = 更新中...
settings-main-status-installed-update = 更新をインストール済み
settings-main-status-cant-install = Warp の新バージョンがありますがインストールできません
settings-main-status-cant-launch = Warp の新バージョンはインストール済みですが起動できません。
settings-main-cta-update-manually = Warp を手動で更新

# --- ANCHOR-SUB-MCP (agent-settings-mcp) ---
# 此锚点下放 settings_view/mcp_servers_page.rs 字符串
# 命名前缀:settings-mcp-*
settings-mcp-page-title = MCP サーバー
settings-mcp-logout-success-named = {$name} MCP サーバーからログアウトしました
settings-mcp-logout-success = MCP サーバーからログアウトしました
settings-mcp-install-modal-busy = 別のインストールリンクを開く前に、現在の MCP インストールを完了してください。
settings-mcp-unknown-server = 不明な MCP サーバー '{$name}'
settings-mcp-install-from-link-failed = このリンクから MCP サーバー '{$name}' をインストールできません。

# ---- destructive_mcp_confirmation_dialog.rs ----
settings-mcp-confirm-delete-local-title = MCP サーバーを削除しますか?
settings-mcp-confirm-delete-local-description = この MCP サーバーをすべてのデバイスからアンインストールして削除します。
settings-mcp-confirm-delete-shared-title = 共有 MCP サーバーを削除しますか?
settings-mcp-confirm-delete-shared-description = 自分自身からだけでなく、Warp およびすべてのチームメイトのデバイスからこの MCP サーバーをアンインストールして削除します。
settings-mcp-confirm-unshare-title = 共有 MCP サーバーをチームから削除しますか?
settings-mcp-confirm-unshare-description = この MCP サーバーを Warp およびすべてのチームメイトのデバイスからアンインストールして削除します。
settings-mcp-confirm-delete-button = MCP を削除
settings-mcp-confirm-remove-from-team-button = チームから削除
settings-mcp-confirm-cancel-button = キャンセル

# ---- edit_page.rs ----
settings-mcp-edit-save = 保存
settings-mcp-edit-edit-variables = 変数を編集
settings-mcp-edit-delete = MCP を削除
settings-mcp-edit-remove-from-team = チームから削除
settings-mcp-edit-editing-disabled-banner = MCP サーバーを編集できるのはチーム管理者と作成者のみです。
settings-mcp-edit-add-new-title = MCP サーバーを追加
settings-mcp-edit-edit-named-title = { $name } MCP サーバーを編集
settings-mcp-edit-edit-title = MCP サーバーを編集
settings-mcp-edit-logout-tooltip = ログアウト
settings-mcp-edit-secrets-error = この MCP サーバーにはシークレットが含まれます。設定 > プライバシーでシークレット秘匿の設定を変更してください。
settings-mcp-edit-no-server-error = MCP サーバーが指定されていません。
settings-mcp-edit-multiple-servers-error = 単一サーバー編集中に複数の MCP サーバーを追加できません。

# ---- installation_modal.rs ----
settings-mcp-install-modal-title = { $name } をインストール
settings-mcp-install-modal-source-shared = チームから共有
settings-mcp-install-modal-source-other-device = 別のデバイスから
settings-mcp-install-modal-cancel = キャンセル
settings-mcp-install-modal-install = インストール
settings-mcp-install-modal-no-server = MCP サーバーが選択されていません

# ---- list_page.rs ----
settings-mcp-list-description = MCP サーバーを追加して Warp エージェントの機能を拡張します。MCP サーバーは標準化されたインターフェースを通じてデータソースやツールをエージェントに公開し、プラグインのように動作します。カスタムサーバーを追加するか、プリセットを使用して人気のサーバーから始められます。チームから共有された MCP サーバーもここで確認できます。
settings-mcp-list-learn-more = 詳細はこちら。
settings-mcp-list-empty-state = MCP サーバーを追加するとここに表示されます。
settings-mcp-list-no-search-results = 検索結果が見つかりませんでした
settings-mcp-list-search-placeholder = MCP サーバーを検索
settings-mcp-list-add-button = 追加
settings-mcp-list-file-based-toggle-label = サードパーティエージェントからサーバーを自動起動
settings-mcp-list-file-based-description = グローバルスコープのサードパーティ AI エージェント設定ファイル(ホームディレクトリ等)から MCP サーバーを自動的に検出して起動します。リポジトリ内で検出されたサーバーは自動起動されず、下の「検出元」セクションで個別に有効化する必要があります。
settings-mcp-list-file-based-supported-providers = 対応プロバイダーを表示。
settings-mcp-list-template-available-to-install = インストール可能
settings-mcp-list-file-based-detected = 設定ファイルから検出
settings-mcp-list-toast-server-updated = MCP サーバーを更新しました
settings-mcp-list-section-my-mcps = マイ MCP
settings-mcp-list-section-shared-by-warp-and-team = Warp と { $name } から共有
settings-mcp-list-section-shared-by-warp-and-other-devices = Warp および別のデバイスから共有
settings-mcp-list-section-shared-from-warp = Warp から共有
settings-mcp-list-section-detected-from = { $provider } から検出
settings-mcp-list-chip-global = グローバル
settings-mcp-list-chip-shared-by-creator = 共有者: { $creator }
settings-mcp-list-chip-shared-by-team-member = チームメンバーから共有
settings-mcp-list-chip-from-another-device = 別のデバイスから

# ---- server_card.rs ----
settings-mcp-card-tooltip-show-logs = ログを表示
settings-mcp-card-tooltip-log-out = ログアウト
settings-mcp-card-tooltip-share-server = サーバーを共有
settings-mcp-card-tooltip-edit = 編集
settings-mcp-card-tooltip-update-available = サーバー更新あり
settings-mcp-card-button-view-logs = ログを表示
settings-mcp-card-button-edit-config = 設定を編集
settings-mcp-card-button-set-up = セットアップ
settings-mcp-card-tools-none = 利用可能なツールなし
settings-mcp-card-tools-available = { $count } ツール利用可能
settings-mcp-card-status-offline = オフライン
settings-mcp-card-status-starting = サーバーを起動中...
settings-mcp-card-status-authenticating = 認証中...
settings-mcp-card-status-shutting-down = シャットダウン中...

# ---- update_modal.rs ----
settings-mcp-update-modal-default-name = サーバー
settings-mcp-update-modal-title = { $name } を更新
settings-mcp-update-modal-description = このサーバーには { $count } 件の更新があります。どれで進めますか?
settings-mcp-update-modal-publisher-another-device = 別のデバイス
settings-mcp-update-modal-publisher-team-member = チームメンバー
settings-mcp-update-modal-update-from = { $publisher } から更新
settings-mcp-update-modal-version = バージョン { $version }
settings-mcp-update-modal-cancel = キャンセル
settings-mcp-update-modal-update = 更新
settings-mcp-update-modal-no-updates = 利用可能な更新はありません

# --- ANCHOR-SUB-PLATFORM (agent-settings-platform) ---
# 此锚点下放 settings_view/platform_page.rs 字符串
# 命名前缀:settings-platform-*
settings-platform-section-title = Oz Cloud API キー
settings-platform-description = API キーを作成・管理して、ローカルエージェントから Warp アカウントへのアクセスを許可します。
    詳細は次を参照
settings-platform-documentation-link = ドキュメント。
settings-platform-create-button = + API キーを作成
settings-platform-modal-title-new = 新しい API キー
settings-platform-modal-title-save = キーを保存
settings-platform-toast-deleted = API キーを削除しました
settings-platform-column-name = 名前
settings-platform-column-key = キー
settings-platform-column-scope = スコープ
settings-platform-column-created = 作成日
settings-platform-column-last-used = 最終使用
settings-platform-column-expires-at = 有効期限
settings-platform-value-never = なし
settings-platform-scope-personal = 個人
settings-platform-scope-team = チーム
settings-platform-zero-state-title = API キーがありません
settings-platform-zero-state-description = キーを作成して Warp への外部アクセスを管理します
settings-platform-create-api-key-description-personal = この API キーはユーザーに紐付き、Warp アカウントに対するリクエストを実行できます。
settings-platform-create-api-key-description-team = この API キーはチームに紐付き、チームを代理してリクエストを実行できます。
settings-platform-create-api-key-name-placeholder = Warp API キー
settings-platform-create-api-key-expiration-one-day = 1 日
settings-platform-create-api-key-expiration-thirty-days = 30 日
settings-platform-create-api-key-expiration-ninety-days = 90 日
settings-platform-create-api-key-label-type = 種別
settings-platform-create-api-key-label-expiration = 有効期限
settings-platform-create-api-key-error-no-current-team = 現在のチームが存在しないため、チーム API キーを作成できません。
settings-platform-create-api-key-error-create-failed = API キーの作成に失敗しました。再試行してください。
settings-platform-create-api-key-secret-once = このシークレットキーは一度しか表示されません。コピーして安全に保管してください。
settings-platform-create-api-key-copied = コピー済み
settings-platform-create-api-key-done = 完了
settings-platform-create-api-key-creating = 作成中…
settings-platform-create-api-key-create = キーを作成
settings-platform-create-api-key-toast-secret-copied = シークレットキーをコピーしました。

# --- ANCHOR-SUB-KEYBINDINGS (agent-settings-keybindings) ---
settings-keybindings-search-placeholder = 名前またはキーで検索 (例: "cmd d")
settings-keybindings-conflict-warning = このショートカットは他のキーバインドと競合しています
settings-keybindings-button-default = デフォルト
settings-keybindings-button-cancel = キャンセル
settings-keybindings-button-clear = クリア
settings-keybindings-button-save = 保存
settings-keybindings-press-new-shortcut = 新しいキーボードショートカットを押してください
settings-keybindings-description = 既存のアクションに独自のキーバインドを追加できます。
settings-keybindings-use-prefix = 使用
settings-keybindings-use-suffix = でいつでもサイドペインからこれらのキーバインドを参照できます。
settings-keybindings-not-synced-tooltip = キーボードショートカットはローカルに保存されます
settings-keybindings-subheader = キーボードショートカットを設定
settings-keybindings-command-column = コマンド

# --- ANCHOR-SUB-REFERRALS (agent-settings-referrals) ---
settings-referrals-page-title = 友人を Warp に招待
settings-referrals-anonymous-header = サインアップして Warp の紹介プログラムに参加
settings-referrals-sign-up = サインアップ
settings-referrals-link-label = リンク
settings-referrals-email-label = メール
settings-referrals-link-error = 紹介コードの読み込みに失敗しました。
settings-referrals-loading = 読み込み中...
settings-referrals-copy-link-button = リンクをコピー
settings-referrals-email-send-button = 送信
settings-referrals-email-sending-button = 送信中...
settings-referrals-link-copied-toast = リンクをコピーしました。
settings-referrals-email-success-toast = メールを送信しました。
settings-referrals-email-failure-toast = メール送信に失敗しました。再試行してください。
settings-referrals-email-empty-error = メールアドレスを入力してください。
settings-referrals-email-invalid-error = 次のメールアドレスが有効か確認してください: { $email }
settings-referrals-reward-intro = 紹介で Warp 限定グッズを獲得*
settings-referrals-claimed-count-singular = 現在の紹介数
settings-referrals-claimed-count-plural = 現在の紹介数
settings-referrals-terms-link = 一部制限が適用されます。
settings-referrals-terms-contact = { " " }紹介プログラムについてのご質問は referrals@warp.dev までご連絡ください。
settings-referrals-reward-theme = 限定テーマ
settings-referrals-reward-keycaps = キーキャップ + ステッカー
settings-referrals-reward-tshirt = T シャツ
settings-referrals-reward-notebook = ノートブック
settings-referrals-reward-cap = キャップ
settings-referrals-reward-hoodie = パーカー
settings-referrals-reward-hydroflask = プレミアム Hydro Flask
settings-referrals-reward-backpack = バックパック

# --- ANCHOR-SUB-WARPIFY (agent-settings-warpify) ---
settings-warpify-page-title = Warpify
settings-warpify-description-prefix = Warp が特定のシェルを「Warpify」(ブロック・入力モード等のサポートを追加) するかを設定します。
settings-warpify-learn-more = 詳細
settings-warpify-section-subshells = サブシェル
settings-warpify-section-subshells-subtitle = 対応サブシェル: bash、zsh、fish。
settings-warpify-section-ssh = SSH
settings-warpify-section-ssh-subtitle = 対話的な SSH セッションを Warpify します。
settings-warpify-added-commands = 追加されたコマンド
settings-warpify-denylisted-commands = 拒否リストのコマンド
settings-warpify-denylisted-hosts = 拒否リストのホスト
settings-warpify-command-placeholder = コマンド (正規表現対応)
settings-warpify-host-placeholder = ホスト (正規表現対応)
settings-warpify-enable-ssh = SSH セッションを Warpify
settings-warpify-install-ssh-extension = SSH 拡張をインストール
settings-warpify-install-ssh-extension-description = リモートホストに Warp の SSH 拡張がインストールされていない場合のインストール挙動を制御します。
settings-warpify-use-tmux = Tmux Warpification を使用
settings-warpify-tmux-description = tmux ssh ラッパーは既定のラッパーが動作しない多くの状況で機能しますが、warpify するためにボタン押下が必要な場合があります。新しいタブから有効になります。
settings-warpify-ssh-tmux-toggle-binding-label = Warpification 用 SSH セッション検出

# --- ANCHOR-SUB-AI-PAGE (agent-settings-ai-page) ---
# Section / sub-headers
settings-ai-warp-agent-header = Warp エージェント
settings-ai-active-ai-section = アクティブな AI
settings-ai-input-section = 入力
settings-ai-mcp-servers-section = MCP サーバー
settings-ai-knowledge-section = ナレッジ
settings-ai-voice-section = 音声
settings-ai-other-section = その他
settings-ai-third-party-cli-section = サードパーティ CLI エージェント
settings-ai-experimental-section = 実験的機能
settings-ai-aws-bedrock-section = AWS Bedrock
settings-ai-agents-header = エージェント
settings-ai-profiles-header = プロファイル
settings-ai-models-subheader = モデル
settings-ai-permissions-subheader = 権限
settings-ai-usage-header = 使用状況
settings-ai-credits-label = クレジット

# Active AI toggle labels
settings-ai-next-command-label = 次のコマンド
settings-ai-prompt-suggestions-label = プロンプト候補
settings-ai-suggested-code-banners-label = コード候補バナー
settings-ai-natural-language-autosuggestions-label = 自然言語オートサジェスト
settings-ai-git-operations-autogen-label = コミット & プルリクエスト生成

# Permissions dropdown options
settings-ai-permission-agent-decides = エージェントが判断
settings-ai-permission-always-allow = 常に許可
settings-ai-permission-always-ask = 常に確認
settings-ai-permission-ask-on-first-write = 初回の書き込み時に確認
settings-ai-permission-read-only = 読み取り専用
settings-ai-permission-supervised = 監督付き
settings-ai-permission-allow-specific-dirs = 特定ディレクトリで許可

# Permission row labels
settings-ai-apply-code-diffs = コード差分の適用
settings-ai-read-files = ファイルの読み取り
settings-ai-execute-commands = コマンドの実行
settings-ai-interact-running-commands = 実行中コマンドとの対話
settings-ai-call-mcp-servers = MCP サーバーの呼び出し
settings-ai-command-denylist = コマンド拒否リスト
settings-ai-command-denylist-description = Warp エージェントが実行する前に必ず権限を確認するコマンドにマッチする正規表現。
settings-ai-command-allowlist = コマンド許可リスト
settings-ai-command-allowlist-description = Warp エージェントが自動的に実行できるコマンドにマッチする正規表現。
settings-ai-directory-allowlist = ディレクトリ許可リスト
settings-ai-directory-allowlist-description = エージェントに特定ディレクトリのファイルアクセスを付与します。
settings-ai-mcp-allowlist = MCP 許可リスト
settings-ai-mcp-allowlist-description = Warp エージェントによるこれらの MCP サーバーの呼び出しを許可します。
settings-ai-mcp-denylist = MCP 拒否リスト
settings-ai-mcp-denylist-description = Warp エージェントはこのリストにある MCP サーバーを呼び出す前に必ず権限を確認します。
settings-ai-info-banner-managed-by-workspace = 一部の権限はワークスペースによって管理されています。

# Models / Profiles
settings-ai-base-model = ベースモデル
settings-ai-base-model-description = このモデルは Warp エージェントの中心エンジンとして機能します。ほとんどのやり取りを担い、必要に応じて計画やコード生成などのタスクで他のモデルを呼び出します。Warp はモデルの可用性や、会話要約などの補助タスクに応じて自動的に別のモデルへ切り替えることがあります。
settings-ai-show-model-picker-in-prompt = プロンプトにモデル選択を表示
settings-ai-codebase-context = コードベースのコンテキスト
settings-ai-codebase-context-description = Warp エージェントによるコードベースの概要生成を許可し、コンテキストとして利用します。コードがサーバーに保存されることはありません。
settings-ai-add-profile = プロファイルを追加
settings-ai-agents-description = エージェントの動作範囲を設定します。アクセスできる対象、自律性のレベル、承認を求めるタイミングを選択できます。自然言語入力、コードベース認識などの動作も細かく調整できます。
settings-ai-profiles-description = プロファイルでは、エージェントが実行できるアクションや承認が必要なタイミング、コーディングや計画などのタスクで使用するモデルなど、エージェントの動作を定義できます。プロジェクトごとにスコープを限定することもできます。

# Anonymous / org gates
settings-ai-sign-up = サインアップ
settings-ai-anonymous-create-account = AI 機能を使うにはアカウントを作成してください。
settings-ai-org-disallows-remote-session = 組織の設定により、アクティブなペインにリモートセッションのコンテンツが含まれる場合は AI を利用できません
settings-ai-org-enforced-tooltip = この項目は組織の設定で強制されており、変更できません。
settings-ai-restricted-billing = 請求の問題により制限中
settings-ai-unlimited = 無制限

# AI Input section
settings-ai-show-input-hint-text = 入力ヒントを表示
settings-ai-show-agent-tips = エージェントのヒントを表示
settings-ai-include-agent-commands-in-history = エージェントが実行したコマンドを履歴に含める
settings-ai-autodetect-agent-prompts = ターミナル入力中のエージェントプロンプトを自動検出
settings-ai-autodetect-terminal-commands = エージェント入力中のターミナルコマンドを自動検出
settings-ai-natural-language-detection = 自然言語の検出
settings-ai-natural-language-denylist = 自然言語拒否リスト
settings-ai-natural-language-denylist-description = ここに登録したコマンドは自然言語検出をトリガーしません。
settings-ai-let-us-know = フィードバックを送る

# MCP Servers
settings-ai-learn-more = 詳細を見る
settings-ai-add-server = サーバーを追加
settings-ai-manage-mcp-servers = MCP サーバーを管理
settings-ai-file-based-mcp-toggle = サードパーティエージェントからサーバーを自動起動
settings-ai-file-based-mcp-supported-providers = 対応プロバイダを見る。
settings-ai-mcp-dropdown-header = MCP サーバーを選択

# Knowledge / Rules
settings-ai-rules-label = ルール
settings-ai-suggested-rules-label = ルールの提案
settings-ai-suggested-rules-description = やり取りに基づいて保存するルールを AI に提案させます。
settings-ai-manage-rules = ルールを管理
settings-ai-rules-description = ルールは、Warp エージェントがコードベースや特定のワークフローのお作法に沿うよう導きます。

# Voice
settings-ai-voice-input-label = 音声入力
settings-ai-voice-key = 音声入力を有効化するキー
settings-ai-voice-key-hint = 押し続けると有効になります。

# Other section
settings-ai-show-use-agent-footer = 「Use Agent」フッターを表示
settings-ai-use-agent-footer-description = 長時間実行コマンドで「フルターミナル使用」を有効化したエージェントを使うヒントを表示します。
settings-ai-show-conversation-history = ツールパネルに会話履歴を表示
settings-ai-thinking-display = エージェントの思考表示
settings-ai-thinking-display-description = 推論や思考のトレースの表示方法を制御します。
settings-ai-conversation-layout-label = 既存のエージェント会話を開く際の優先レイアウト
settings-ai-conversation-layout-newtab = 新しいタブ
settings-ai-conversation-layout-splitpane = 分割ペイン
settings-ai-toolbar-layout = ツールバーのレイアウト

# Third-party CLI agents
settings-ai-show-coding-agent-toolbar = コーディングエージェントのツールバーを表示
settings-ai-auto-show-rich-input = エージェントの状態に応じて Rich Input を自動表示/非表示
settings-ai-auto-show-rich-input-tooltip = コーディングエージェント用の Warp プラグインが必要です
settings-ai-auto-open-rich-input = コーディングエージェントのセッション開始時に Rich Input を自動で開く
settings-ai-auto-dismiss-rich-input = プロンプト送信後に Rich Input を自動で閉じる
settings-ai-toolbar-commands-label = ツールバーを有効化するコマンド
settings-ai-toolbar-commands-description = マッチするコマンドでコーディングエージェントツールバーを表示する正規表現を追加します。
settings-ai-coding-agent-other = その他
settings-ai-coding-agent-select-header = コーディングエージェントを選択

# Experimental / Agent
settings-ai-cloud-agent-computer-use = エージェントでのコンピュータ操作
settings-ai-cloud-agent-computer-use-description = Warp アプリから開始したエージェント会話でコンピュータ操作を有効化します。
settings-ai-orchestration-label = オーケストレーション
settings-ai-orchestration-description = マルチエージェントオーケストレーションを有効化し、エージェントが並列のサブエージェントを起動・調整できるようにします。

# AWS Bedrock
settings-ai-aws-bedrock-toggle = AWS Bedrock の認証情報を使用
settings-ai-aws-bedrock-description = Warp はローカルの AWS CLI 認証情報を読み込み、Bedrock 対応モデルへ送信します。
settings-ai-aws-bedrock-description-managed = Warp はローカルの AWS CLI 認証情報を読み込み、Bedrock 対応モデルへ送信します。この設定は組織によって管理されています。
settings-ai-aws-login-command = ログインコマンド
settings-ai-aws-profile = AWS プロファイル
settings-ai-aws-auto-login = ログインコマンドを自動実行
settings-ai-aws-auto-login-description = 有効にすると、AWS Bedrock の認証情報が期限切れになった際にログインコマンドが自動で実行されます。
settings-ai-refresh = 更新

# --- ANCHOR-SUB-FEATURES (agent-settings-features) ---
# settings_view/features_page.rs P0 + P1(category + toggle labels)
# 命名前缀:settings-features-*
settings-features-category-general = 一般
settings-features-category-session = セッション
settings-features-category-keys = キー
settings-features-category-text-editing = テキスト編集
settings-features-category-terminal-input = ターミナル入力
settings-features-category-terminal = ターミナル
settings-features-category-notifications = 通知
settings-features-category-workflows = ワークフロー
settings-features-category-system = システム
settings-features-open-links-in-desktop = リンクをデスクトップアプリで開く
settings-features-open-links-in-desktop-tooltip = 可能な限りリンクを自動的にデスクトップアプリで開きます。
settings-features-restore-session = 起動時にウィンドウ・タブ・ペインを復元
settings-features-persist-conversations = エージェント会話をローカル履歴に保存
settings-features-show-sticky-command-header = 固定コマンドヘッダを表示
settings-features-show-link-tooltip = リンククリック時にツールチップを表示
settings-features-show-quit-warning = 終了/ログアウト前に警告を表示
settings-features-quit-on-last-window-closed = すべてのウィンドウを閉じたら終了
settings-features-show-changelog-after-update = アップデート後にチェンジログのトーストを表示
settings-features-mouse-scroll-multiplier = マウスホイール 1 回でスクロールする行数
settings-features-auto-open-code-review = コードレビューパネルを自動で開く
settings-features-max-rows-per-block = ブロック内の最大行数
settings-features-ssh-wrapper = Warp SSH ラッパー
settings-features-receive-desktop-notifications = Warp からのデスクトップ通知を受信
settings-features-show-in-app-agent-notifications = アプリ内エージェント通知を表示
settings-features-confirm-close-shared-session = 共有セッションを閉じる前に確認
settings-features-global-hotkey-label = グローバルホットキー:
settings-features-global-hotkey-not-supported-on-wayland = Wayland では未対応です。
settings-features-autocomplete-symbols = クォート・括弧・カッコを自動補完
settings-features-error-underlining = コマンドのエラー下線
settings-features-syntax-highlighting = コマンドの構文ハイライト
settings-features-completions-while-typing = 入力中に補完メニューを開く
settings-features-command-corrections = 修正コマンドの提案
settings-features-expand-aliases = 入力中にエイリアスを展開
settings-features-middle-click-paste = 中クリックで貼り付け
settings-features-vim-mode = Vim キーバインドでコードとコマンドを編集
settings-features-at-context-menu = ターミナルモードで '@' コンテキストメニューを有効化
settings-features-slash-commands-in-terminal = ターミナルモードでスラッシュコマンドを有効化
settings-features-outline-codebase-symbols = '@' コンテキストメニュー用にコードベースのシンボルをアウトライン化
settings-features-show-input-message-bar = ターミナル入力メッセージ行を表示
settings-features-show-autosuggestion-hint = オートサジェストのキーバインドヒントを表示
settings-features-show-autosuggestion-ignore = オートサジェストの無視ボタンを表示
settings-features-enable-mouse-reporting = マウスレポートを有効化
settings-features-enable-scroll-reporting = スクロールレポートを有効化
settings-features-enable-focus-reporting = フォーカスレポートを有効化
settings-features-use-audible-bell = 可聴ベルを使用
settings-features-double-click-smart-selection = ダブルクリックでスマート選択
settings-features-show-help-block-in-new-sessions = 新規セッションでヘルプブロックを表示
settings-features-copy-on-select = 選択でコピー
settings-features-show-global-workflows-in-command-search = コマンド検索 (ctrl-r) にグローバルワークフローを表示
settings-features-linux-selection-clipboard = Linux 選択クリップボードを尊重
settings-features-prefer-low-power-gpu = 新規ウィンドウは内蔵 GPU (低消費電力) で描画する
settings-features-use-wayland = ウィンドウ管理に Wayland を使用
settings-features-use-wayland-tooltip = Wayland の使用を有効化します
settings-features-ctrl-tab-behavior-label = Ctrl+Tab の動作:
settings-features-extra-meta-key-left-mac = 左 Option キーを Meta にする
settings-features-extra-meta-key-right-mac = 右 Option キーを Meta にする
settings-features-extra-meta-key-left-other = 左 Alt キーを Meta にする
settings-features-extra-meta-key-right-other = 右 Alt キーを Meta にする
settings-features-default-shell-header = 新規セッションの既定シェル
settings-features-working-directory-header = 新規セッションの作業ディレクトリ
settings-features-notify-agent-task-completed = エージェントがタスクを完了したら通知
settings-features-notify-needs-attention = コマンドやエージェントが続行のために注意を必要とするときに通知
settings-features-play-notification-sounds = 通知音を鳴らす
settings-features-default-session-mode = 新規セッションの既定モード
settings-features-block-rows-description = 上限を 10 万行を超えて設定するとパフォーマンスに影響する場合があります。サポートされる最大行数は { $max_rows } です。
settings-features-toast-duration-label = トースト通知を表示し続ける時間
settings-features-tab-key-behavior = Tab キーの動作
settings-features-graphics-backend-label = 優先するグラフィックスバックエンド
settings-features-graphics-backend-current = 現在のバックエンド: { $backend }
settings-features-working-dir-home = ホームディレクトリ
settings-features-working-dir-previous = 前回セッションのディレクトリ
settings-features-working-dir-custom = カスタムディレクトリ
settings-features-undo-close-enable = 閉じたセッションの再オープンを有効化
settings-features-undo-close-grace-period = 猶予期間 (秒)
settings-features-configure-global-hotkey = グローバルホットキーを設定
settings-features-make-default-terminal = Warp を既定のターミナルにする
settings-features-pin-top = 上に固定
settings-features-pin-bottom = 下に固定
settings-features-pin-left = 左に固定
settings-features-pin-right = 右に固定
settings-features-default-option = 既定
settings-features-tab-behavior-completions = 補完メニューを開く
settings-features-tab-behavior-autosuggestions = オートサジェストを採用
settings-features-tab-behavior-user-defined = ユーザー定義
settings-features-new-tab-placement-all = すべてのタブの後ろ
settings-features-new-tab-placement-current = 現在のタブの後ろ
settings-features-width-percent = 幅 %
settings-features-height-percent = 高さ %
settings-features-autohide-on-focus-loss = キーボードフォーカスを失ったら自動非表示
settings-features-long-running-prefix = コマンドの完了に
settings-features-long-running-suffix = 秒以上かかるとき
settings-features-keybinding-label = キーバインド
settings-features-click-set-global-hotkey = クリックしてグローバルホットキーを設定
settings-features-cancel = キャンセル
settings-features-save = 保存
settings-features-press-new-shortcut = 新しいショートカットキーを押してください
settings-features-change-keybinding = キーバインドを変更
settings-features-active-screen = アクティブな画面
settings-features-wayland-window-restore-warning = Wayland ではウィンドウ位置は復元されません。
settings-features-see-docs = ドキュメントを参照。
settings-features-allowed-values-1-20 = 許容値: 1〜20
settings-features-supports-floating-1-20 = 1〜20 の浮動小数点値に対応します。
settings-features-auto-open-code-review-description = この設定が有効な場合、会話で最初に受け入れた差分でコードレビューパネルが開きます
settings-features-default-terminal-current = Warp が既定のターミナルです
settings-features-takes-effect-new-sessions = この変更は新規セッションから有効になります
settings-features-seconds = 秒
settings-features-vim-system-clipboard = 無名レジスタをシステムクリップボードに設定
settings-features-vim-status-bar = Vim ステータスバーを表示
settings-features-tab-behavior-right-arrow-accepts = → でオートサジェストを採用します。
settings-features-tab-behavior-key-accepts = { $keybinding } でオートサジェストを採用します。
settings-features-completions-open-while-typing-sentence = 入力に応じて補完が開きます。
settings-features-completions-open-while-typing-or-key = 入力に応じて補完が開きます ({ $keybinding } でも可)。
settings-features-open-completions-unbound = 補完メニューを開く操作は割り当てられていません。
settings-features-tab-behavior-key-opens-completions = { $keybinding } で補完メニューを開きます。
settings-features-word-characters-label = 単語の一部とみなす文字
settings-features-new-tab-placement = 新規タブの配置
settings-features-linux-selection-clipboard-tooltip = Linux のプライマリクリップボードをサポートするかどうか。
settings-features-changes-apply-new-windows = 変更は新規ウィンドウに適用されます。
settings-features-wayland-description = この設定を有効にするとグローバルホットキーは使えなくなります。無効の場合、Wayland コンポジタが分数スケーリング (例: 125%) を使用しているとテキストがぼやけることがあります。
settings-features-restart-warp-to-apply = 変更を反映するには Warp を再起動してください。

# --- ANCHOR-SUB-SETTINGS-PAGE-NAV (agent-settings-page-nav) ---
# 此锚点下放 settings_view/{settings_page,nav,delete_environment_confirmation_dialog,directory_color_add_picker,pane_manager}.rs 字符串
# 命名前缀:settings-page-* / settings-nav-* / settings-confirm-* / settings-color-picker-*

# ---- settings_page.rs ----
settings-page-info-icon-tooltip = クリックしてドキュメントで詳細を見る
settings-page-local-only-icon-tooltip = この設定は他のデバイスとは同期されません
settings-page-reset-to-default = 既定値に戻す

# ---- delete_environment_confirmation_dialog.rs ----
settings-confirm-cancel = キャンセル
settings-confirm-delete-environment-button = 環境を削除
settings-confirm-delete-environment-title = 環境を削除しますか？
settings-confirm-delete-environment-description = { $name } 環境を削除してもよろしいですか？

# ---- directory_color_add_picker.rs ----
settings-color-picker-add-directory-footer = + ディレクトリを追加…
settings-color-picker-add-directory-color = ディレクトリの色を追加

# ---- settings_file_footer.rs ----
settings-footer-open-file = 設定ファイルを開く
settings-footer-alert-open-file = ファイルを開く
settings-footer-alert-fix-with-oz = Oz で修正

# --- ANCHOR-SUB-CODE (agent-settings-code) ---
settings-code-auto-open-review-panel = コードレビューパネルを自動で開く
settings-code-auto-open-review-panel-desc = この設定が有効な場合、会話で最初に承認された差分時にコードレビューパネルが開きます
settings-code-show-code-review-button = コードレビューボタンを表示
settings-code-show-code-review-button-desc = ウィンドウ右上にコードレビューパネルを切り替えるボタンを表示します。
settings-code-show-diff-stats = コードレビューボタンに差分統計を表示
settings-code-show-diff-stats-desc = コードレビューボタンに追加・削除行数を表示します。
settings-code-project-explorer = プロジェクトエクスプローラー
settings-code-project-explorer-desc = 左側ツールパネルに IDE スタイルのプロジェクトエクスプローラー / ファイルツリーを追加します。
settings-code-global-search = グローバルファイル検索
settings-code-global-search-desc = 左側ツールパネルにグローバルファイル検索を追加します。

# --- ANCHOR-SUB-PRIVACY (agent-settings-privacy) ---
settings-privacy-page-title = プライバシー
settings-privacy-modal-add-regex-title = 正規表現パターンを追加
settings-privacy-safe-mode-title = シークレットの伏字化
settings-privacy-safe-mode-description = この設定が有効な場合、Warp はブロック、Warp Drive オブジェクトの内容、Oz プロンプトに含まれる機密情報の可能性をスキャンし、サーバーへの保存・送信を防止します。正規表現でリストをカスタマイズできます。
settings-privacy-user-secret-regex-title = カスタムシークレット伏字化
settings-privacy-user-secret-regex-description = 正規表現で追加で伏字化したいシークレットやデータを定義します。次のコマンド実行時から反映されます。正規表現の先頭に (?i) フラグを付けると大文字小文字を無視できます。
settings-privacy-telemetry-title = Warp の改善に協力する
settings-privacy-telemetry-description = アプリ分析は製品改善に役立ちます。Warp の AI 機能を改善するため、特定のコンソール操作を収集する場合があります。
settings-privacy-telemetry-description-old = アプリ分析は製品改善に役立ちます。アプリ使用メタデータのみを収集し、コンソールの入出力は収集しません。
settings-privacy-telemetry-free-tier-note = 無料プランでは AI 機能を使用するために分析を有効にする必要があります。
settings-privacy-telemetry-docs-link = Warp のデータ利用について詳しく見る
settings-privacy-data-management-title = データの管理
settings-privacy-data-management-description = いつでも Warp アカウントを完全に削除できます。削除後は Warp を使用できなくなります。
settings-privacy-data-management-link = データ管理ページを開く
settings-privacy-policy-title = プライバシーポリシー
settings-privacy-policy-link = Warp のプライバシーポリシーを読む
settings-privacy-tab-personal = 個人
settings-privacy-tab-enterprise = エンタープライズ
settings-privacy-enterprise-readonly = エンタープライズのシークレット伏字化は変更できません。
settings-privacy-enterprise-empty = 組織で設定されたエンタープライズ正規表現はありません。
settings-privacy-recommended = 推奨
settings-privacy-add-all = すべて追加
settings-privacy-add-regex-button = 正規表現を追加
settings-privacy-enterprise-enabled-by-org = 組織により有効化されています。
settings-privacy-zdr-badge = ZDR
settings-privacy-zdr-tooltip = 管理者がチームに対しゼロデータ保持を有効にしています。ユーザー生成コンテンツは収集されません。
settings-privacy-secret-display-mode-title = シークレットの視覚的伏字化モード
settings-privacy-secret-display-mode-description = 検索可能性を保ちつつ、ブロック一覧でシークレットがどのように表示されるかを選択します。この設定はブロック一覧の表示のみに影響します。
settings-privacy-crash-reports-title = クラッシュレポートを送信
settings-privacy-crash-reports-description = クラッシュレポートはデバッグと安定性向上に役立ちます。
settings-privacy-cloud-conv-title = AI 会話をローカルに保存
settings-privacy-cloud-conv-description-on = エージェント会話を他者と共有でき、別のデバイスでログインしても保持されます。このデータは製品機能のためのみに保存され、Warp は分析には使用しません。
settings-privacy-cloud-conv-description-off = エージェント会話はこのマシン上にローカル保存され、外部サービスへ同期されません。
settings-privacy-org-managed-tooltip = この設定は組織により管理されています。
settings-privacy-network-log-title = ネットワークログコンソール
settings-privacy-network-log-description = Warp から外部サーバーへのすべての通信を確認できるネイティブコンソールを構築しました。作業内容が常に安全に保たれていることを確認できます。
settings-privacy-network-log-link = ネットワークログを表示

# --- ANCHOR-SUB-EXEC-MODAL-BLOCKS (agent-settings-misc) ---
# ---- execution_profile_view ----
settings-exec-profile-edit-button = 編集
settings-exec-profile-auto = 自動
settings-exec-profile-section-models = モデル
settings-exec-profile-section-permissions = 権限
settings-exec-profile-base-model = ベースモデル:
settings-exec-profile-full-terminal-use = ターミナル全面利用:
settings-exec-profile-title-model = タイトル生成:
settings-exec-profile-active-ai-model = アクティブ AI:
settings-exec-profile-next-command-model = 次のコマンド:
settings-exec-profile-computer-use = コンピュータ操作:
settings-exec-profile-apply-code-diffs = コード差分の適用:
settings-exec-profile-read-files = ファイル読み取り:
settings-exec-profile-execute-commands = コマンド実行:
settings-exec-profile-interact-running-commands = 実行中コマンドとの対話:
settings-exec-profile-ask-questions = 質問:
settings-exec-profile-call-mcp-servers = MCP サーバー呼び出し:
settings-exec-profile-call-web-tools = Web ツール呼び出し:
settings-exec-profile-chips-none = なし
settings-exec-profile-perm-agent-decides = エージェントが判断
settings-exec-profile-perm-always-allow = 常に許可
settings-exec-profile-perm-always-ask = 常に確認
settings-exec-profile-perm-unknown = 不明
settings-exec-profile-perm-ask-on-first-write = 初回書き込み時に確認
settings-exec-profile-perm-never = 許可しない
settings-exec-profile-perm-never-ask = 確認しない
settings-exec-profile-perm-ask-unless-auto-approve = 自動承認以外は確認
settings-exec-profile-perm-on = オン
settings-exec-profile-perm-off = オフ
settings-exec-profile-directory-allowlist = ディレクトリ許可リスト:
settings-exec-profile-command-allowlist = コマンド許可リスト:
settings-exec-profile-command-denylist = コマンド拒否リスト:
settings-exec-profile-mcp-allowlist = MCP 許可リスト:
settings-exec-profile-mcp-denylist = MCP 拒否リスト:

# ---- execution_profile_editor (Profile Editor pane) ----
settings-exec-profile-editor-header = プロファイルエディタ
settings-exec-profile-editor-title = プロファイルを編集
settings-exec-profile-editor-name-label = 名前
settings-exec-profile-editor-default-name-info = 既定プロファイル名は変更できません。
settings-exec-profile-editor-workspace-override-tooltip = このオプションは組織の設定により強制されており、カスタマイズできません。
settings-exec-profile-editor-section-models = モデル
settings-exec-profile-editor-section-permissions = 権限
settings-exec-profile-editor-base-model = ベースモデル
settings-exec-profile-editor-base-model-desc = このモデルはエージェントの主要エンジンとして機能します。ほとんどのやり取りを駆動し、必要に応じて計画やコード生成などのために他モデルを呼び出します。Warp はモデルの可用性や、会話の要約などの補助タスクのため、自動的に代替モデルに切り替える場合があります。
settings-exec-profile-editor-full-terminal-use-model = ターミナル全面利用モデル
settings-exec-profile-editor-full-terminal-use-model-desc = データベースシェル、デバッガ、REPL、開発サーバーなど、対話型ターミナルアプリケーション内でエージェントが動作する際に使用されるモデル。ライブ出力を読み取り、PTY にコマンドを書き込みます。
settings-exec-profile-editor-title-model = タイトル生成モデル
settings-exec-profile-editor-title-model-desc = 簡潔な会話タイトルを生成するために使用されるモデル。既定はベースモデル。エージェント本体の推論に影響を与えずにタイトル要約のトークンを節約するため、より安価/高速なモデルを選択できます。
settings-exec-profile-editor-active-ai-model = アクティブ AI モデル
settings-exec-profile-editor-active-ai-model-desc = プロアクティブ AI 機能で使用されるモデル。コマンド完了後のプロンプト提案、エージェント入力での自然言語オートコンプリート、コードベースの関連度ランキング。既定はベースモデル。エージェント本体の推論に影響を与えずに機能を軽快に保つため、小型/高速なモデルを選択できます。
settings-exec-profile-editor-next-command-model = 次のコマンドモデル
settings-exec-profile-editor-next-command-model-desc = 次のシェルコマンドを予測するために使用されるモデル(グレーのインライン自動候補 + ブロック完了後のゼロ状態提案)。レイテンシ重視 — 利用可能な最小/最速の BYOP モデルを選択してください。既定はベースモデル。
settings-exec-profile-editor-computer-use-model = コンピュータ操作モデル
settings-exec-profile-editor-computer-use-model-desc = エージェントがコンピュータを制御し、マウス操作、クリック、キーボード入力を通じてグラフィカルアプリケーションと対話する際に使用されるモデル。
settings-exec-profile-editor-apply-code-diffs = コード差分の適用
settings-exec-profile-editor-read-files = ファイル読み取り
settings-exec-profile-editor-execute-commands = コマンド実行
settings-exec-profile-editor-interact-running-commands = 実行中コマンドとの対話
settings-exec-profile-editor-computer-use = コンピュータ操作
settings-exec-profile-editor-ask-questions = 質問
settings-exec-profile-editor-call-mcp-servers = MCP サーバー呼び出し
settings-exec-profile-editor-call-web-tools = Web ツール呼び出し
settings-exec-profile-editor-call-web-tools-desc = エージェントはタスクの完了に役立つ場合に Web 検索を使用できます。
settings-exec-profile-editor-directory-allowlist = ディレクトリ許可リスト
settings-exec-profile-editor-directory-allowlist-desc = エージェントに特定ディレクトリへのファイルアクセスを与えます。
settings-exec-profile-editor-command-allowlist = コマンド許可リスト
settings-exec-profile-editor-command-allowlist-desc = Oz が自動実行できるコマンドにマッチさせる正規表現。
settings-exec-profile-editor-command-denylist = コマンド拒否リスト
settings-exec-profile-editor-command-denylist-desc = Oz が常に実行許可を求めるべきコマンドにマッチさせる正規表現。
settings-exec-profile-editor-mcp-allowlist = MCP 許可リスト
settings-exec-profile-editor-mcp-allowlist-desc = Oz による呼び出しが許可される MCP サーバー。
settings-exec-profile-editor-mcp-denylist = MCP 拒否リスト
settings-exec-profile-editor-mcp-denylist-desc = Oz による呼び出しが許可されない MCP サーバー。

# ---- agent_assisted_environment_modal ----
settings-env-modal-add-repo = リポジトリを追加
settings-env-modal-cancel = キャンセル
settings-env-modal-create-environment = 環境を作成
settings-env-modal-selected-repos = 選択済みリポジトリ
settings-env-modal-no-repos-selected = まだリポジトリが選択されていません
settings-env-modal-available-repos = 利用可能なインデックス済みリポジトリ
settings-env-modal-loading = ローカルでインデックス化されたリポジトリを読み込み中…
settings-env-modal-empty-no-indexed = ローカルでインデックス化されたリポジトリが見つかりません。リポジトリをインデックス化してから再試行してください。
settings-env-modal-unavailable-build = このビルドではローカルリポジトリの選択は利用できません。
settings-env-modal-all-selected = ローカルでインデックス化されたすべてのリポジトリは既に選択されています。
settings-env-modal-unknown-repo-name = (不明)
settings-env-modal-not-git-repo = 選択したフォルダは Git リポジトリではありません: { $path }
settings-env-modal-no-directory-selected = ディレクトリが選択されていません
settings-env-modal-dialog-title = 環境用のリポジトリを選択
settings-env-modal-dialog-description-indexed = 環境作成エージェントへ文脈を提供するため、ローカルでインデックス化されたリポジトリを選択してください。
settings-env-modal-dialog-description-default = 環境作成エージェントへ文脈を提供するため、リポジトリを選択してください。

# ---- show_blocks_view ----
settings-show-blocks-page-title = 共有ブロック
settings-show-blocks-unshare-menu-item = 共有解除
settings-show-blocks-copy-link = リンクをコピー
settings-show-blocks-deleting = 削除中...
settings-show-blocks-executed-on = 実行日時: { $time }
settings-show-blocks-empty = 共有ブロックはまだありません。
settings-show-blocks-loading = ブロックを取得中...
settings-show-blocks-load-failed = ブロックの読み込みに失敗しました。再試行してください。
settings-show-blocks-link-copied = リンクをコピーしました。
settings-show-blocks-unshare-success = ブロックを共有解除しました。
settings-show-blocks-unshare-failed = ブロックの共有解除に失敗しました。再試行してください。
settings-show-blocks-confirm-dialog-title = ブロックを共有解除
settings-show-blocks-confirm-dialog-text = このブロックを共有解除してもよろしいですか?

    リンク経由でアクセスできなくなり、Warp サーバーから完全に削除されます。
settings-show-blocks-confirm-cancel = キャンセル
settings-show-blocks-confirm-unshare = 共有解除

# --- ANCHOR-SUB-APPEARANCE (agent-settings-appearance) ---
# 此锚点下放 settings_view/appearance_page.rs 剩余字符串(不含已完成的 Language widget)
# 命名前缀:settings-appearance-*

# Categories
settings-appearance-category-themes = テーマ
settings-appearance-category-language = 言語
settings-appearance-category-icon = アイコン
settings-appearance-category-window = ウィンドウ
settings-appearance-category-input = 入力
settings-appearance-category-panes = ペイン
settings-appearance-category-blocks = ブロック
settings-appearance-category-text = テキスト
settings-appearance-category-cursor = カーソル
settings-appearance-category-tabs = タブ
settings-appearance-category-fullscreen-apps = 全画面アプリ

# Theme widget
settings-appearance-theme-create-custom = 独自のカスタムテーマを作成
settings-appearance-theme-mode-light = ライト
settings-appearance-theme-mode-dark = ダーク
settings-appearance-theme-mode-current = 現在のテーマ
settings-appearance-theme-sync-os-label = OS と同期
settings-appearance-theme-sync-os-description = システムに合わせてライトテーマとダークテーマを自動的に切り替えます。

# Custom App Icon widget
settings-appearance-custom-icon-label = アプリアイコンをカスタマイズ
settings-appearance-custom-icon-bundle-warning = アプリアイコンの変更にはアプリがバンドルされている必要があります。
settings-appearance-custom-icon-restart-warning = MacOS で希望のアイコンスタイルを適用するには Warp の再起動が必要な場合があります。

# Window widgets
settings-appearance-window-custom-size-label = カスタムサイズで新規ウィンドウを開く
settings-appearance-window-columns-label = 列数
settings-appearance-window-rows-label = 行数
settings-appearance-window-opacity-label = ウィンドウの不透明度:
settings-appearance-window-opacity-value = ウィンドウの不透明度: { $value }
settings-appearance-window-opacity-not-supported = 透明化はお使いのグラフィックドライバではサポートされていません。
settings-appearance-window-opacity-graphics-warning = 選択中のグラフィック設定では透明ウィンドウのレンダリングがサポートされない場合があります。
settings-appearance-window-opacity-graphics-warning-hint = 機能 > システムでグラフィックバックエンドまたは統合 GPU の設定を変更してみてください。
settings-appearance-window-blur-radius = ウィンドウぼかし半径: { $value }
settings-appearance-window-blur-texture-label = ウィンドウぼかしを使用 (Acrylic テクスチャ)
settings-appearance-window-blur-texture-not-supported = 選択中のハードウェアでは透明ウィンドウのレンダリングがサポートされない場合があります。
settings-appearance-tools-panel-consistent-label = ツールパネルの表示状態をタブ間で統一する

# Input
settings-appearance-input-type-label = 入力タイプ
settings-appearance-input-type-warp = Warp
settings-appearance-input-type-shell = シェル (PS1)
settings-appearance-input-position-label = 入力位置
settings-appearance-input-mode-pinned-bottom = 下部に固定 (Warp モード)
settings-appearance-input-mode-pinned-top = 上部に固定 (リバースモード)
settings-appearance-input-mode-waterfall = 上から開始 (クラシックモード)

# Panes
settings-appearance-pane-dim-inactive-label = 非アクティブなペインを暗くする
settings-appearance-pane-focus-follows-mouse-label = マウスにフォーカスが追従する

# Blocks
settings-appearance-block-compact-label = コンパクトモード
settings-appearance-block-jump-bottom-label = ブロック下部へジャンプボタンを表示
settings-appearance-block-show-dividers-label = ブロックの区切り線を表示

# Text / Fonts
settings-appearance-font-agent-label = エージェントフォント
settings-appearance-font-match-terminal = ターミナルに合わせる
settings-appearance-font-terminal-label = ターミナルフォント
settings-appearance-font-view-all-system = 利用可能なシステムフォントをすべて表示
settings-appearance-font-weight-label = フォントの太さ
settings-appearance-font-size-label = フォントサイズ (px)
settings-appearance-font-line-height-label = 行の高さ
settings-appearance-font-reset-default = 既定にリセット
settings-appearance-font-notebook-size-label = ノートブックフォントサイズ
settings-appearance-font-thin-strokes-label = 細いストロークを使用
settings-appearance-font-thin-strokes-never = 使用しない
settings-appearance-font-thin-strokes-low-dpi = 低 DPI ディスプレイで使用
settings-appearance-font-thin-strokes-high-dpi = 高 DPI ディスプレイで使用
settings-appearance-font-thin-strokes-always = 常に使用
settings-appearance-font-min-contrast-label = 最低コントラストを強制
settings-appearance-font-min-contrast-always = 常に
settings-appearance-font-min-contrast-named-only = 名前付き色のみ
settings-appearance-font-min-contrast-never = 適用しない
settings-appearance-font-ligatures-label = ターミナルでリガチャを表示
settings-appearance-font-ligatures-perf-tooltip = リガチャはパフォーマンスを低下させる場合があります

# Cursor
settings-appearance-cursor-type-label = カーソルタイプ
settings-appearance-cursor-disabled-vim = Vim モードではカーソルタイプは無効です
settings-appearance-cursor-blink-label = カーソルを点滅させる

# Tabs
settings-appearance-tab-close-position-label = タブ閉じるボタンの位置
settings-appearance-tab-close-position-right = 右
settings-appearance-tab-close-position-left = 左
settings-appearance-tab-show-indicators-label = タブインジケータを表示
settings-appearance-tab-show-code-review-label = コードレビューボタンを表示
settings-appearance-tab-preserve-active-color-label = 新規タブにアクティブタブの色を引き継ぐ
settings-appearance-tab-vertical-layout-label = 垂直タブレイアウトを使用
settings-appearance-tab-show-vertical-panel-in-restored-windows-label = 復元されたウィンドウで垂直タブパネルを表示
settings-appearance-tab-show-vertical-panel-in-restored-windows-description = 有効にすると、ウィンドウの再オープン・復元時に、最後の保存時に垂直タブパネルが閉じていても開いて表示します。
settings-appearance-tab-show-title-bar-search-bar-label = タイトルバーに検索バーを表示
settings-appearance-tab-show-title-bar-search-bar-description = タイトルバー中央に「セッション、エージェント、ファイルを検索…」検索バーを表示します。クリックでコマンドパレットが開きます。無効化するとスロットが空になります。垂直タブレイアウトのみに適用されます。
settings-appearance-tab-use-prompt-as-title-label = タブ名に最新のユーザープロンプトを会話タイトルとして使用
settings-appearance-tab-use-prompt-as-title-description = 垂直タブのビルトイン AI およびサードパーティエージェントセッションで、生成された会話タイトルの代わりに最新のユーザープロンプトを表示します。
settings-appearance-tab-toolbar-layout-label = ヘッダーツールバーレイアウト
settings-appearance-tab-directory-colors-label = ディレクトリタブの色
settings-appearance-tab-directory-colors-description = 作業中のディレクトリやリポジトリに基づいてタブを自動的に色分けします。
settings-appearance-tab-directory-color-default-tooltip = 既定 (色なし)
settings-appearance-zen-mode-label = タブバーを表示
settings-appearance-zen-decoration-always = 常に
settings-appearance-zen-decoration-windowed = ウィンドウ表示時
settings-appearance-zen-decoration-on-hover = ホバー時のみ

# Full-screen apps
settings-appearance-alt-screen-padding-label = alt-screen でカスタムパディングを使用
settings-appearance-alt-screen-uniform-padding-label = 均一パディング (px)

# Zoom
settings-appearance-zoom-label = ズーム
settings-appearance-zoom-secondary = すべてのウィンドウの既定ズームレベルを調整します

# --- ANCHOR-SUB-ENVIRONMENTS (agent-settings-environments) ---
settings-environments-page-title = 環境
settings-environments-page-description = 環境はアンビエントエージェントが実行される場所を定義します。GitHub (推奨)、Warp 支援セットアップ、または手動構成で数分で設定できます。
settings-environments-search-placeholder = 環境を検索...
settings-environments-no-matches = 検索条件に一致する環境はありません。
settings-environments-section-personal = 個人
settings-environments-section-team-default = Warp とチームで共有
settings-environments-section-team-named = Warp と { $team } で共有
settings-environments-env-id-prefix = 環境 ID: { $id }
settings-environments-detail-image = イメージ: { $image }
settings-environments-detail-repos = リポジトリ: { $repos }
settings-environments-detail-setup-commands = セットアップコマンド: { $commands }
settings-environments-last-edited = 最終編集: { $time }
settings-environments-last-used = 最終使用: { $time }
settings-environments-last-used-never = 最終使用: なし
settings-environments-view-my-runs = 自分の実行を表示
settings-environments-tooltip-share = 共有
settings-environments-tooltip-edit = 編集
settings-environments-empty-header = まだ環境がセットアップされていません。
settings-environments-empty-subheader = 環境のセットアップ方法を選択してください:
settings-environments-empty-quick-setup-title = クイックセットアップ
settings-environments-empty-suggested-badge = 推奨
settings-environments-empty-quick-setup-subtitle = 作業したい GitHub リポジトリを選択すると、ベースイメージと構成を提案します
settings-environments-empty-use-agent-title = エージェントを使用
settings-environments-empty-use-agent-subtitle = ローカルでセットアップ済みのプロジェクトを選択すると、それに基づいた環境セットアップを支援します
settings-environments-button-loading = 読み込み中...
settings-environments-button-retry = 再試行
settings-environments-button-authorize = 認可
settings-environments-button-get-started = 開始
settings-environments-button-launch-agent = エージェント起動
settings-environments-toast-update-success = 環境を更新しました
settings-environments-toast-create-success = 環境を作成しました
settings-environments-toast-delete-success = 環境を削除しました
settings-environments-toast-share-success = 環境を共有しました
settings-environments-toast-share-failure = チームへの環境共有に失敗しました
settings-environments-toast-create-not-logged-in = 環境を作成できません: ログインしていません。
settings-environments-toast-save-not-found = 保存できません: 環境が存在しません。
settings-environments-toast-share-no-team = 環境を共有できません: 現在チームに所属していません。
settings-environments-toast-share-not-synced = 環境を共有できません: 環境がまだ同期されていません。
settings-update-environment-name-placeholder = 環境名
settings-update-environment-docker-image-placeholder = 例: python:3.11、node:20-alpine
settings-update-environment-repos-placeholder-authed = リポジトリを入力 (owner/repo 形式)
settings-update-environment-repos-placeholder-unauthenticated = リポジトリ URL を貼り付け
settings-update-environment-setup-command-placeholder = 例: cd my-repo && pip install -r requirements.txt
settings-update-environment-description-placeholder = 例: この環境はフロントエンド中心のエージェント全般用です

# --- ANCHOR-SUB-AGENT-PROVIDERS (agent-settings-agent-providers) ---
# 此锚点下放 settings_view/agent_providers_widget.rs 字符串
# 命名前缀:settings-agent-providers-*
settings-agent-providers-title = エージェントプロバイダー
settings-agent-providers-description = OpenAI 互換 (DeepSeek、Zhipu GLM、Moonshot、DashScope、SiliconFlow、OpenRouter など)、Anthropic、Gemini、ローカル Ollama など、複数プロトコルにまたがるカスタムエージェントプロバイダーを構成します。モデルは手動 (表示名 + モデル ID マッピング) または API から自動取得で追加できます。プロバイダーのメタデータはローカルの settings.toml に保存され、API キーはシステムキーチェーンに安全に保存されます。
settings-agent-providers-empty = プロバイダーがまだ構成されていません。右上の [+ プロバイダー追加] をクリックして追加してください。
settings-agent-providers-add-button = + プロバイダー追加
settings-agent-providers-search-placeholder = プロバイダーを検索…
settings-agent-providers-quick-add-title = クイック追加
settings-agent-providers-refresh-catalog = カタログを更新
settings-agent-providers-loading-catalog = models.dev カタログを読み込み中… (初回読み込みは数秒かかる場合があります)
settings-agent-providers-catalog-empty = models.dev カタログが空です。[カタログを更新] をクリックして再試行してください。
settings-agent-providers-no-match = 「{ $query }」に一致する項目はありません
settings-agent-providers-collapse = 折りたたむ ▲
settings-agent-providers-expand-remaining = 残り { $count } 件を展開 ▼
settings-agent-providers-row-missing = (このプロバイダーに紐付くエディタはまだありません: { $id })
settings-agent-providers-field-name = 名前
settings-agent-providers-field-base-url = ベース URL
settings-agent-providers-field-api-key = API キー
settings-agent-providers-field-api-type = API タイプ
settings-agent-providers-api-type-hint = (genai はこれを使ってアダプタを明示的にバインドし、モデル名による誤検出を回避します。ベース URL が空の場合、既定値が使用されます: { $url })
settings-agent-providers-name-placeholder = カスタムプロバイダー名 (例: DeepSeek、ローカル Ollama)
settings-agent-providers-api-key-placeholder = sk-... (フォーカスを外すか Enter でシステムキーチェーンに保存)
settings-agent-providers-models-label = モデル ({ $count })
settings-agent-providers-models-empty-hint = モデルがまだ構成されていません。[+ モデル追加] で手動追加、または [API から取得] で自動取得してください。
settings-agent-providers-models-header-name = 表示名
settings-agent-providers-models-header-id = モデル ID
settings-agent-providers-models-header-context = コンテキスト (tok)
settings-agent-providers-models-header-output = 出力 (tok)
settings-agent-providers-model-name-placeholder = 表示名 (例: DS-V3 General)
settings-agent-providers-model-id-placeholder = モデル ID (API に送信される `model` フィールド、例: deepseek-chat)
settings-agent-providers-model-context-placeholder = コンテキスト (トークン)
settings-agent-providers-model-output-placeholder = 出力 (トークン)
settings-agent-providers-add-model = + モデル追加
settings-agent-providers-fetch-from-api = API から取得
settings-agent-providers-sync-models-dev = models.dev から同期
settings-agent-providers-remove = 削除

# ---- AI page (settings_view/ai_page.rs) ----
settings-ai-title = AI
settings-ai-active-ai = アクティブ AI
settings-ai-input-autodetection = エージェント入力でのターミナルコマンド自動検出
settings-ai-input-autodetection-legacy = 自然言語検出
settings-ai-next-command-description = コマンド履歴、出力、一般的なワークフローに基づき、次に実行するコマンドを AI に提案させます。
settings-ai-prompt-suggestions-description = 最近のコマンドと出力に基づき、入力欄のインラインバナーとして AI に自然言語プロンプトを提案させます。
settings-ai-suggested-code-banners-description = 最近のコマンドと出力に基づき、ブロック一覧のインラインバナーとして AI にコード差分とクエリを提案させます。
settings-ai-natural-language-autosuggestions = 最近のコマンドと出力に基づき、AI に自然言語の自動候補を提案させます。
settings-ai-git-operations-autogen-description = コミットメッセージ、プルリクエストのタイトルと説明を AI に生成させます。
# =============================================================================
# SECTION: ai (Owner: agent-ai)
# Files: app/src/ai/**, app/src/ai_assistant/**
# =============================================================================

# (placeholder — to be filled by agent-ai)

# =============================================================================
# SECTION: command-palette (Owner: agent-cmdpal)
# Files: app/src/command_palette.rs, app/src/palette/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: drive (Owner: agent-drive)
# Files: app/src/drive/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: workspace (Owner: agent-workspace)
# Files: app/src/workspace/**, app/src/workspaces/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: modal (Owner: agent-modal)
# Files: app/src/modal/**, app/src/prompt/**, app/src/quit_warning/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: auth (Owner: agent-auth)
# Files: app/src/auth/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: banner (Owner: agent-banner)
# Files: app/src/banner/**
# =============================================================================

banner-dont-show-again = 今後表示しない

# =============================================================================
# =============================================================================
# SECTION: quit-warning (Owner: agent-quit-warning)
# Files: app/src/quit_warning/mod.rs
# =============================================================================

# ---- Dialog titles ----
quit-warning-title-pane = ペインを閉じますか?
quit-warning-title-tab-singular = タブを閉じますか?
quit-warning-title-tab-plural = タブを閉じますか?
quit-warning-title-window = ウィンドウを閉じますか?
quit-warning-title-app = Warp を終了しますか?
quit-warning-title-editor-tab = 変更を保存しますか?

# ---- Buttons ----
quit-warning-button-confirm-close = はい、閉じる
quit-warning-button-confirm-quit = はい、終了
quit-warning-button-save = 保存
quit-warning-button-discard = 保存しない
quit-warning-button-show-processes = 実行中のプロセスを表示
quit-warning-button-cancel = キャンセル

# ---- Warning body lines ----
# Suffix appended to each warning line, indicating the scope.
quit-warning-suffix-tab = { " " }(このタブ内)。
quit-warning-suffix-window = { " " }(このウィンドウ内)。
quit-warning-suffix-pane = { " " }(このペイン内)。
quit-warning-suffix-default = 。

# Process info: "{count} process(es) running" with optional window/tab qualifier.
quit-warning-processes-running = { $count } { $count ->
        [one] 個のプロセスが実行中
       *[other] 個のプロセスが実行中
    }です
quit-warning-processes-in-windows = { " " }({ $count } 個のウィンドウ)
quit-warning-processes-in-tabs = { " " }({ $count } 個のタブ)

# Shared sessions line.
quit-warning-shared-sessions = { $count } { $count ->
        [one] 個のセッション
       *[other] 個のセッション
    }を共有中です

# Unsaved code changes (generic scope).
quit-warning-unsaved-changes = 未保存のファイル変更があります

# Unsaved code changes for a specific editor tab.
quit-warning-unsaved-editor-tab = { $file } への変更を保存しますか? 保存しない場合、変更は破棄されます。
quit-warning-unsaved-editor-tab-fallback-name = このファイル

# --- ANCHOR-SUB-RULES-PAGE (agent-rules-page) ---
# Manage Rules 页面(Warp Drive 中的 AI Fact Collection)。
rules-collection-name = ルール

# --- ANCHOR-SUB-KEYBINDING-DESC (agent-keybinding-descriptions) ---
# Description 文案 for keyboard binding entries shown in the Settings >
# Keyboard Shortcuts page and the command palette. Each key corresponds to
# a binding registered via `EditableBinding::new(name, description, action)`
# or `BindingDescription::new("…")`. The binding `name` (e.g.
# `workspace:open_settings_file`) is **not** translated — it is a protocol
# field used to persist user-customised shortcuts.

# Tabs / sessions
keybinding-desc-workspace-cycle-next-session = 次のタブに切り替え
keybinding-desc-workspace-cycle-prev-session = 前のタブに切り替え
keybinding-desc-workspace-add-window = 新規ウィンドウを作成
keybinding-desc-workspace-new-file = 新規ファイル
keybinding-desc-workspace-zoom-in = 拡大
keybinding-desc-workspace-zoom-out = 縮小
keybinding-desc-workspace-reset-zoom = ズームをリセット
keybinding-desc-workspace-increase-font-size = フォントサイズを大きく
keybinding-desc-workspace-decrease-font-size = フォントサイズを小さく
keybinding-desc-workspace-reset-font-size = フォントサイズを既定にリセット
keybinding-desc-workspace-increase-zoom = ズームレベルを上げる
keybinding-desc-workspace-decrease-zoom = ズームレベルを下げる
keybinding-desc-workspace-reset-zoom-level = ズームレベルを既定にリセット
keybinding-desc-workspace-save-launch-config = 新規 launch 設定を保存

# Project Explorer / panels
keybinding-desc-workspace-toggle-project-explorer = プロジェクトエクスプローラーを切り替え
keybinding-desc-workspace-toggle-project-explorer-menu = プロジェクトエクスプローラー
keybinding-desc-workspace-show-theme-chooser = テーマピッカーを開く
keybinding-desc-workspace-toggle-tab-configs-menu = タブ設定メニューを開く

# Switch to N-th tab
keybinding-desc-workspace-activate-1st-tab = 1番目のタブに切り替え
keybinding-desc-workspace-activate-2nd-tab = 2番目のタブに切り替え
keybinding-desc-workspace-activate-3rd-tab = 3番目のタブに切り替え
keybinding-desc-workspace-activate-4th-tab = 4番目のタブに切り替え
keybinding-desc-workspace-activate-5th-tab = 5番目のタブに切り替え
keybinding-desc-workspace-activate-6th-tab = 6番目のタブに切り替え
keybinding-desc-workspace-activate-7th-tab = 7番目のタブに切り替え
keybinding-desc-workspace-activate-8th-tab = 8番目のタブに切り替え
keybinding-desc-workspace-activate-last-tab = 最後のタブに切り替え
keybinding-desc-workspace-activate-prev-tab = 前のタブをアクティブ化
keybinding-desc-workspace-activate-next-tab = 次のタブをアクティブ化

# Pane navigation
keybinding-desc-pane-group-navigate-prev = 前のペインをアクティブ化
keybinding-desc-pane-group-navigate-next = 次のペインをアクティブ化

# Mouse / Notebooks / Workflows / Folders
keybinding-desc-workspace-toggle-mouse-reporting = マウスレポートを切り替え
keybinding-desc-workspace-create-personal-notebook = 新規個人ノートブックを作成
keybinding-desc-workspace-create-personal-notebook-menu = 新規個人ノートブック
keybinding-desc-workspace-create-personal-workflow = 新規個人ワークフローを作成
keybinding-desc-workspace-create-personal-workflow-menu = 新規個人ワークフロー
keybinding-desc-workspace-create-personal-folder = 新規個人フォルダを作成
keybinding-desc-workspace-create-personal-folder-menu = 新規個人フォルダ

# New tab variants
keybinding-desc-workspace-new-tab = 新規タブを作成
keybinding-desc-workspace-new-terminal-tab = 新規ターミナルタブ
keybinding-desc-workspace-new-agent-tab = 新規エージェントタブ
keybinding-desc-workspace-new-cloud-agent-tab = 新規エージェントタブ

# Left / right panel toggles
keybinding-desc-workspace-toggle-left-panel = 左パネルを開く
keybinding-desc-workspace-toggle-right-panel = コードレビューを切り替え
keybinding-desc-workspace-toggle-right-panel-menu = コードレビューを切り替え
keybinding-desc-workspace-toggle-vertical-tabs = 縦タブパネルを切り替え
keybinding-desc-workspace-toggle-vertical-tabs-menu = 縦タブパネルを切り替え
keybinding-desc-workspace-left-panel-agent-conversations = 左パネル: エージェント会話
keybinding-desc-workspace-left-panel-project-explorer = 左パネル: プロジェクトエクスプローラー
keybinding-desc-workspace-left-panel-global-search = 左パネル: グローバル検索
keybinding-desc-workspace-left-panel-warp-drive = 左パネル: Warp Drive
keybinding-desc-workspace-left-panel-ssh-manager = 左パネル: SSH マネージャー
keybinding-desc-workspace-open-global-search = グローバル検索を開く
keybinding-desc-workspace-open-global-search-menu = グローバル検索
keybinding-desc-workspace-toggle-warp-drive = Warp Drive を切り替え
keybinding-desc-workspace-toggle-warp-drive-menu = Warp Drive
keybinding-desc-workspace-toggle-conversation-list-view = エージェント会話リストビューを切り替え
keybinding-desc-workspace-toggle-conversation-list-view-menu = エージェント会話リストビュー
keybinding-desc-workspace-close-panel = フォーカス中のパネルを閉じる

# Command palette / navigation
keybinding-desc-workspace-toggle-command-palette = コマンドパレットを切り替え
keybinding-desc-workspace-toggle-command-palette-menu = コマンドパレット
keybinding-desc-workspace-toggle-navigation-palette = ナビゲーションパレットを切り替え
keybinding-desc-workspace-toggle-navigation-palette-menu = ナビゲーションパレット
keybinding-desc-workspace-toggle-launch-config-palette = launch 設定パレット
keybinding-desc-workspace-toggle-files-palette = ファイルパレットを切り替え
keybinding-desc-workspace-search-drive = Warp Drive を検索
keybinding-desc-workspace-move-tab-left = タブを左に移動
keybinding-desc-workspace-move-tab-up = タブを上に移動
keybinding-desc-workspace-move-tab-right = タブを右に移動
keybinding-desc-workspace-move-tab-down = タブを下に移動

# Keybindings settings
keybinding-desc-workspace-toggle-keybindings-page = キーボードショートカットを切り替え
keybinding-desc-workspace-show-keybinding-settings = キーバインドエディタを開く
keybinding-desc-workspace-toggle-block-snackbar = スティッキーコマンドヘッダーを切り替え

# Window / tab close
keybinding-desc-workspace-rename-active-tab = 現在のタブをリネーム
keybinding-desc-workspace-terminate-app = Warp を終了
keybinding-desc-workspace-close-window = ウィンドウを閉じる
keybinding-desc-workspace-close-active-tab = 現在のタブを閉じる
keybinding-desc-workspace-close-other-tabs = 他のタブを閉じる
keybinding-desc-workspace-close-tabs-right = 右側のタブを閉じる
keybinding-desc-workspace-close-tabs-below = 下のタブを閉じる

# Notifications
keybinding-desc-workspace-toggle-notifications-on = 通知をオンにする
keybinding-desc-workspace-toggle-notifications-off = 通知をオフにする

# Updates / changelog
keybinding-desc-workspace-update-and-relaunch = アップデートをインストールして再起動
keybinding-desc-workspace-check-for-updates = アップデートを確認
keybinding-desc-workspace-view-changelog = 最新の changelog を表示

# Resource center / Drive export / CLI
keybinding-desc-workspace-toggle-resource-center = リソースセンターを切り替え
keybinding-desc-workspace-export-all-warp-drive-objects = すべての Warp Drive オブジェクトをエクスポート
keybinding-desc-workspace-install-cli = Oz CLI コマンドをインストール
keybinding-desc-workspace-uninstall-cli = Oz CLI コマンドをアンインストール

# AI assistant / agents
keybinding-desc-workspace-toggle-ai-assistant = Warp AI を切り替え

# Env vars / prompts
keybinding-desc-workspace-create-personal-env-vars = 新規個人環境変数を作成
keybinding-desc-workspace-create-personal-env-vars-menu = 新規個人環境変数
keybinding-desc-workspace-create-personal-ai-prompt = 新規個人プロンプトを作成
keybinding-desc-workspace-create-personal-ai-prompt-menu = 新規個人プロンプト

# Focus / import
keybinding-desc-workspace-shift-focus-left = フォーカスを左パネルに切り替え
keybinding-desc-workspace-shift-focus-right = フォーカスを右パネルに切り替え
keybinding-desc-workspace-import-to-personal-drive = 個人 Drive にインポート

# Drive / repository / AI rules / MCP
keybinding-desc-workspace-open-repository = リポジトリを開く
keybinding-desc-workspace-open-repository-menu = リポジトリを開く
keybinding-desc-workspace-open-ai-fact-collection = AI ルールを開く
keybinding-desc-workspace-open-mcp-servers = MCP サーバーを開く
keybinding-desc-workspace-jump-to-latest-toast = 最新のエージェントタスクへジャンプ
keybinding-desc-workspace-toggle-notification-mailbox = 通知メールボックスを切り替え
keybinding-desc-workspace-toggle-agent-management-view = エージェント管理ビューを切り替え

# Settings pages
keybinding-desc-workspace-show-settings = 設定を開く
keybinding-desc-workspace-show-settings-menu = 設定
keybinding-desc-workspace-show-settings-account = 設定を開く: アカウント
keybinding-desc-workspace-show-settings-appearance = 設定を開く: 外観
keybinding-desc-workspace-show-settings-appearance-menu = 外観...
keybinding-desc-workspace-show-settings-features = 設定を開く: 機能
keybinding-desc-workspace-show-settings-shared-blocks = 設定を開く: 共有ブロック
keybinding-desc-workspace-show-settings-shared-blocks-menu = 共有ブロックを表示...
keybinding-desc-workspace-show-settings-keyboard-shortcuts = 設定を開く: キーボードショートカット
keybinding-desc-workspace-show-settings-keyboard-shortcuts-menu = キーボードショートカットを設定...
keybinding-desc-workspace-show-settings-about = 設定を開く: バージョン情報
keybinding-desc-workspace-show-settings-about-menu = Warp について
keybinding-desc-workspace-show-settings-privacy = 設定を開く: プライバシー
keybinding-desc-workspace-show-settings-warpify = 設定を開く: Warpify
keybinding-desc-workspace-show-settings-warpify-menu = Warpify を設定...
keybinding-desc-workspace-show-settings-ai = 設定を開く: AI
keybinding-desc-workspace-show-settings-code = 設定を開く: コード
keybinding-desc-workspace-show-settings-referrals = 設定を開く: リファラル
keybinding-desc-workspace-show-settings-environments = 設定を開く: 環境
keybinding-desc-workspace-show-settings-mcp-servers = 設定を開く: MCP サーバー
keybinding-desc-workspace-open-settings-file = 設定ファイルを開く

# Overflow menu / external links
keybinding-desc-workspace-link-to-slack = Slack コミュニティに参加 (外部リンクを開く)
keybinding-desc-workspace-link-to-user-docs = ユーザードキュメントを表示 (外部リンクを開く)
keybinding-desc-workspace-send-feedback = フィードバックを送信 (外部リンクを開く)
keybinding-desc-workspace-send-feedback-oz = Oz でフィードバックを送信
keybinding-desc-workspace-view-logs = Warp ログを表示
keybinding-desc-workspace-link-to-privacy-policy = プライバシーポリシーを表示 (外部リンクを開く)

# Input / terminal / project bindings (registered outside workspace/mod.rs)
keybinding-desc-input-edit-prompt = プロンプトを編集
keybinding-desc-terminal-attach-block-as-context = 選択ブロックをエージェントコンテキストとして添付
keybinding-desc-terminal-attach-text-as-context = 選択テキストをエージェントコンテキストとして添付
keybinding-desc-terminal-attach-as-context-menu = 選択をエージェントコンテキストとして添付
keybinding-desc-workspace-init-project = warp 用にプロジェクトを初期化
keybinding-desc-workspace-add-current-folder = 現在のフォルダをプロジェクトとして追加

# Workspace debug / crash / sentry / heap profile bindings
keybinding-desc-workspace-crash-macos = アプリをクラッシュさせる (sentry-cocoa テスト用)
keybinding-desc-workspace-crash-other = アプリをクラッシュさせる (sentry-native テスト用)
keybinding-desc-workspace-log-review-comment-send-status = [Debug] アクティブタブのレビューコメント送信状況をログ出力
keybinding-desc-workspace-panic = panic を発生させる (sentry-rust テスト用)
keybinding-desc-workspace-open-view-tree-debugger = ビューツリーデバッガーを開く
keybinding-desc-workspace-view-first-time-user-experience = [Debug] 初回ユーザー体験を表示
keybinding-desc-workspace-open-build-plan-migration-modal = [Debug] Build Plan Migration モーダルを開く
keybinding-desc-workspace-reset-build-plan-migration-modal-state = [Debug] Build Plan Migration モーダル状態をリセット
keybinding-desc-workspace-undismiss-aws-login-banner = [Debug] AWS ログインバナーの非表示を解除
keybinding-desc-workspace-open-oz-launch-modal = [Debug] Oz Launch モーダルを開く
keybinding-desc-workspace-reset-oz-launch-modal-state = [Debug] Oz Launch モーダル状態をリセット
keybinding-desc-workspace-open-openwarp-launch-modal = [Debug] OpenWarp Launch モーダルを開く
keybinding-desc-workspace-reset-openwarp-launch-modal-state = [Debug] OpenWarp Launch モーダル状態をリセット
keybinding-desc-workspace-install-opencode-warp-plugin = [Debug] OpenCode Warp プラグインをインストール
keybinding-desc-workspace-use-local-opencode-warp-plugin = [Debug] ローカル OpenCode Warp プラグインを使用 (テスト専用)
keybinding-desc-workspace-open-session-config-modal = [Debug] Session Config モーダルを開く
keybinding-desc-workspace-start-hoa-onboarding-flow = [Debug] HOA オンボーディングフローを開始
keybinding-desc-workspace-sample-process = プロセスをサンプリング
keybinding-desc-workspace-dump-heap-profile = ヒーププロファイルをダンプ (一度のみ実行可能)

# Terminal input bindings
keybinding-desc-input-show-network-log = Warp ネットワークログを表示
keybinding-desc-input-clear-screen = 画面をクリア
keybinding-desc-input-toggle-classic-completions = (実験的) クラシック補完モードを切り替え
keybinding-desc-input-command-search = コマンド検索
keybinding-desc-input-history-search = 履歴検索
keybinding-desc-input-open-completions-menu = 補完メニューを開く
keybinding-desc-input-workflows = ワークフロー
keybinding-desc-input-open-ai-command-suggestions = AI コマンドサジェストを開く
keybinding-desc-input-new-agent-conversation = 新規エージェント会話
keybinding-desc-input-trigger-auto-detection = 自動検出をトリガー
keybinding-desc-input-clear-and-reset-ai-context-menu-query = AI コンテキストメニューのクエリをクリア・リセット

# Terminal view bindings
keybinding-desc-terminal-alternate-paste = 代替ターミナルペースト
keybinding-desc-terminal-toggle-cli-agent-rich-input = CLI エージェントのリッチ入力を切り替え
keybinding-desc-terminal-warpify-subshell = サブシェルを Warpify
keybinding-desc-terminal-warpify-ssh-session = SSH セッションを Warpify
keybinding-desc-terminal-accept-prompt-suggestion = プロンプトサジェストを承認
keybinding-desc-terminal-cancel-process-windows = テキストをコピーまたは実行中プロセスをキャンセル
keybinding-desc-terminal-cancel-process = 実行中プロセスをキャンセル
keybinding-desc-terminal-focus-input = ターミナル入力にフォーカス
keybinding-desc-terminal-paste = 貼り付け
keybinding-desc-terminal-copy = コピー
keybinding-desc-terminal-reinput-commands = 選択コマンドを再入力
keybinding-desc-terminal-reinput-commands-sudo = 選択コマンドを root として再入力
keybinding-desc-terminal-find = ターミナル内を検索
keybinding-desc-terminal-select-bookmark-up = 上の最も近いブックマークを選択
keybinding-desc-terminal-select-bookmark-down = 下の最も近いブックマークを選択
keybinding-desc-terminal-open-block-context-menu = ブロックコンテキストメニューを開く
keybinding-desc-terminal-toggle-workflows-modal = ワークフローモーダルを切り替え
keybinding-desc-terminal-copy-git-branch = git ブランチをコピー
keybinding-desc-terminal-clear-blocks = ブロックをクリア
keybinding-desc-terminal-cursor-word-left = 実行中コマンド内でカーソルを 1 単語左へ
keybinding-desc-terminal-cursor-word-right = 実行中コマンド内でカーソルを 1 単語右へ
keybinding-desc-terminal-cursor-home = 実行中コマンド内でカーソルを行頭へ
keybinding-desc-terminal-cursor-end = 実行中コマンド内でカーソルを行末へ
keybinding-desc-terminal-delete-word-left = 実行中コマンド内で左の単語を削除
keybinding-desc-terminal-delete-line-start = 実行中コマンド内で行頭まで削除
keybinding-desc-terminal-delete-line-end = 実行中コマンド内で行末まで削除
keybinding-desc-terminal-backward-tabulation = 実行中コマンド内で逆方向タブ
keybinding-desc-terminal-select-previous-block = 前のブロックを選択
keybinding-desc-terminal-select-next-block = 次のブロックを選択
keybinding-desc-terminal-share-selected-block = 選択ブロックを共有
keybinding-desc-terminal-bookmark-selected-block = 選択ブロックをブックマーク
keybinding-desc-terminal-find-within-selected-block = 選択ブロック内を検索
keybinding-desc-terminal-copy-command-and-output = コマンドと出力をコピー
keybinding-desc-terminal-copy-command-output = コマンド出力をコピー
keybinding-desc-terminal-copy-command = コマンドをコピー
keybinding-desc-terminal-scroll-up-one-line = ターミナル出力を 1 行上にスクロール
keybinding-desc-terminal-scroll-down-one-line = ターミナル出力を 1 行下にスクロール
keybinding-desc-terminal-scroll-to-top-of-block = 選択ブロックの先頭にスクロール
keybinding-desc-terminal-scroll-to-bottom-of-block = 選択ブロックの末尾にスクロール
keybinding-desc-terminal-select-all-blocks = すべてのブロックを選択
keybinding-desc-terminal-expand-blocks-above = 選択ブロックを上方向に展開
keybinding-desc-terminal-expand-blocks-below = 選択ブロックを下方向に展開
keybinding-desc-terminal-insert-command-correction = コマンド訂正を挿入
keybinding-desc-terminal-setup-guide = セットアップガイド
keybinding-desc-terminal-onboarding-warp-input-terminal = [Debug] オンボーディング吹き出し: WarpInput - Terminal
keybinding-desc-terminal-onboarding-warp-input-project = [Debug] オンボーディング吹き出し: WarpInput - Project
keybinding-desc-terminal-onboarding-warp-input-no-project = [Debug] オンボーディング吹き出し: WarpInput - No Project
keybinding-desc-terminal-onboarding-modality-project = [Debug] オンボーディング吹き出し: Modality - Project
keybinding-desc-terminal-onboarding-modality-no-project = [Debug] オンボーディング吹き出し: Modality - No Project
keybinding-desc-terminal-onboarding-modality-terminal = [Debug] オンボーディング吹き出し: Modality - Terminal
keybinding-desc-terminal-import-external-settings = 外部設定をインポート
keybinding-desc-terminal-share-current-session = 現在のセッションを共有
keybinding-desc-terminal-stop-sharing-current-session = 現在のセッションの共有を停止
keybinding-desc-terminal-toggle-block-filter = 選択中または最後のブロックでブロックフィルタを切り替え
keybinding-desc-terminal-toggle-sticky-command-header = アクティブペインのスティッキーコマンドヘッダーを切り替え
keybinding-desc-terminal-toggle-autoexecute-mode = 自動実行モードを切り替え
keybinding-desc-terminal-toggle-queue-next-prompt = 次プロンプトのキューを切り替え

# Pane group bindings
keybinding-desc-pane-group-close-current-session = 現在のセッションを閉じる
keybinding-desc-pane-group-split-left = ペインを左に分割
keybinding-desc-pane-group-split-up = ペインを上に分割
keybinding-desc-pane-group-split-down = ペインを下に分割
keybinding-desc-pane-group-split-right = ペインを右に分割
keybinding-desc-pane-group-switch-left = 左のペインに切り替え
keybinding-desc-pane-group-switch-right = 右のペインに切り替え
keybinding-desc-pane-group-switch-up = 上のペインに切り替え
keybinding-desc-pane-group-switch-down = 下のペインに切り替え
keybinding-desc-pane-group-resize-left = ペインのサイズ変更 > 仕切りを左へ
keybinding-desc-pane-group-resize-right = ペインのサイズ変更 > 仕切りを右へ
keybinding-desc-pane-group-resize-up = ペインのサイズ変更 > 仕切りを上へ
keybinding-desc-pane-group-resize-down = ペインのサイズ変更 > 仕切りを下へ
keybinding-desc-pane-group-toggle-maximize = アクティブペインの最大化を切り替え

# Root view bindings
keybinding-desc-root-view-toggle-fullscreen = フルスクリーンを切り替え
keybinding-desc-root-view-enter-onboarding-state = [Debug] オンボーディング状態に入る

# Workflow view bindings
keybinding-desc-workflow-view-save = ワークフローを保存
keybinding-desc-workflow-view-close = 閉じる

# Editor view binding desc (shared by editor/view/mod.rs, code/editor/view/actions.rs, notebooks/editor/view.rs)
keybinding-desc-editor-copy = コピー
keybinding-desc-editor-cut = カット
keybinding-desc-editor-paste = 貼り付け
keybinding-desc-editor-undo = 元に戻す
keybinding-desc-editor-redo = やり直す
keybinding-desc-editor-select-left-by-word = 1 単語左を選択
keybinding-desc-editor-select-right-by-word = 1 単語右を選択
keybinding-desc-editor-select-left = 1 文字左を選択
keybinding-desc-editor-select-right = 1 文字右を選択
keybinding-desc-editor-select-up = 上を選択
keybinding-desc-editor-select-down = 下を選択
keybinding-desc-editor-select-all = すべて選択
keybinding-desc-editor-select-to-line-start = 行頭まで選択
keybinding-desc-editor-select-to-line-end = 行末まで選択
keybinding-desc-editor-select-to-line-start-cap = 行頭まで選択
keybinding-desc-editor-select-to-line-end-cap = 行末まで選択
keybinding-desc-editor-clear-and-copy-lines = 選択行をコピーしてクリア
keybinding-desc-editor-add-next-occurrence = 次の出現箇所を選択に追加
keybinding-desc-editor-up = カーソルを上へ
keybinding-desc-editor-down = カーソルを下へ
keybinding-desc-editor-left = カーソルを左へ
keybinding-desc-editor-right = カーソルを右へ
keybinding-desc-editor-move-to-line-start = 行頭へ移動
keybinding-desc-editor-move-to-line-end = 行末へ移動
keybinding-desc-editor-move-to-line-start-short = 行頭へ移動
keybinding-desc-editor-move-to-line-end-short = 行末へ移動
keybinding-desc-editor-home = Home
keybinding-desc-editor-end = End
keybinding-desc-editor-cmd-down = カーソルを最下部へ
keybinding-desc-editor-cmd-up = カーソルを最上部へ
keybinding-desc-editor-move-to-and-select-buffer-start = 最上部まで選択して移動
keybinding-desc-editor-move-to-and-select-buffer-end = 最下部まで選択して移動
keybinding-desc-editor-move-forward-one-word = 1 単語前進
keybinding-desc-editor-move-backward-one-word = 1 単語後退
keybinding-desc-editor-move-forward-one-word-cap = 1 単語前進
keybinding-desc-editor-move-backward-one-word-cap = 1 単語後退
keybinding-desc-editor-move-to-paragraph-start = 段落の先頭へ移動
keybinding-desc-editor-move-to-paragraph-end = 段落の末尾へ移動
keybinding-desc-editor-move-to-paragraph-start-short = 段落の先頭へ移動
keybinding-desc-editor-move-to-paragraph-end-short = 段落の末尾へ移動
keybinding-desc-editor-move-to-buffer-start = バッファの先頭へ移動
keybinding-desc-editor-move-to-buffer-end = バッファの末尾へ移動
keybinding-desc-editor-cursor-at-buffer-start = カーソルをバッファ先頭に
keybinding-desc-editor-cursor-at-buffer-end = カーソルをバッファ末尾に
keybinding-desc-editor-backspace = 前の文字を削除
keybinding-desc-editor-cut-word-left = 左の単語をカット
keybinding-desc-editor-cut-word-right = 右の単語をカット
keybinding-desc-editor-delete-word-left = 左の単語を削除
keybinding-desc-editor-delete-word-right = 右の単語を削除
keybinding-desc-editor-cut-all-left = 左側すべてをカット
keybinding-desc-editor-cut-all-right = 右側すべてをカット
keybinding-desc-editor-delete-all-left = 左側すべてを削除
keybinding-desc-editor-delete-all-right = 右側すべてを削除
keybinding-desc-editor-delete = 削除
keybinding-desc-editor-clear-lines = 選択行をクリア
keybinding-desc-editor-insert-newline = 改行を挿入
keybinding-desc-editor-fold = 折りたたむ
keybinding-desc-editor-unfold = 展開する
keybinding-desc-editor-fold-selected-ranges = 選択範囲を折りたたむ
keybinding-desc-editor-insert-last-word-prev-cmd = 前のコマンドの最後の単語を挿入
keybinding-desc-editor-move-backward-one-subword = 1 サブワード後退
keybinding-desc-editor-move-forward-one-subword = 1 サブワード前進
keybinding-desc-editor-select-left-by-subword = 1 サブワード左を選択
keybinding-desc-editor-select-right-by-subword = 1 サブワード右を選択
keybinding-desc-editor-accept-autosuggestion = オートサジェストを承認
keybinding-desc-editor-inspect-command = コマンドを検査
keybinding-desc-editor-clear-buffer = コマンドエディタをクリア
keybinding-desc-editor-add-cursor-above = 上にカーソルを追加
keybinding-desc-editor-add-cursor-below = 下にカーソルを追加
keybinding-desc-editor-insert-nonexpanding-space = 非展開スペースを挿入
keybinding-desc-editor-vim-exit-insert-mode = Vim 挿入モードを抜ける
keybinding-desc-editor-toggle-comment = コメントを切り替え
keybinding-desc-editor-go-to-line = 行へ移動
keybinding-desc-editor-find-in-code-editor = コードエディタ内を検索

# Code editor (Code) binding desc
keybinding-desc-code-save-as = 名前を付けてファイルを保存
keybinding-desc-code-close-all-tabs = すべてのタブを閉じる
keybinding-desc-code-close-saved-tabs = 保存済みタブを閉じる

# Welcome view binding desc
keybinding-desc-welcome-terminal-session = ターミナルセッション
keybinding-desc-welcome-add-repository = リポジトリを追加

# AI assistant panel binding desc
keybinding-desc-ai-assistant-close = Warp AI を閉じる
keybinding-desc-ai-assistant-focus-terminal-input = Warp AI からターミナル入力にフォーカス
keybinding-desc-ai-assistant-restart = Warp AI を再起動

# Code review binding desc
keybinding-desc-code-review-save-all = コードレビューの未保存ファイルをすべて保存
keybinding-desc-code-review-show-find = コードレビューに検索バーを表示

# Project buttons binding desc
keybinding-desc-project-buttons-open-repository = リポジトリを開く
keybinding-desc-project-buttons-create-new-project = 新規プロジェクトを作成

# Find view binding desc
keybinding-desc-find-next-occurrence = 検索クエリの次の出現箇所を検索
keybinding-desc-find-prev-occurrence = 検索クエリの前の出現箇所を検索

# Notebook file / notebook binding desc
keybinding-desc-notebook-focus-terminal-input-from-file = ファイルからターミナル入力にフォーカス
keybinding-desc-notebook-reload-file = ファイルを再読み込み
keybinding-desc-notebook-increase-font-size = ノートブックフォントサイズを大きく
keybinding-desc-notebook-decrease-font-size = ノートブックフォントサイズを小さく
keybinding-desc-notebook-reset-font-size = ノートブックフォントサイズをリセット
keybinding-desc-notebook-focus-terminal-input = ノートブックからターミナル入力にフォーカス
keybinding-desc-notebook-fb-increase-font-size = フォントサイズを大きく
keybinding-desc-notebook-fb-decrease-font-size = フォントサイズを小さく

# Notebook editor binding desc (extra to shared editor keys)
keybinding-desc-nbeditor-deselect-command = シェルコマンドの選択を解除
keybinding-desc-nbeditor-select-command = カーソル位置のシェルコマンドを選択
keybinding-desc-nbeditor-select-previous-command = 前のコマンドを選択
keybinding-desc-nbeditor-select-next-command = 次のコマンドを選択
keybinding-desc-nbeditor-run-commands = 選択コマンドを実行
keybinding-desc-nbeditor-toggle-debug = リッチテキストデバッグモードを切り替え
keybinding-desc-nbeditor-debug-copy-buffer = リッチテキストバッファをコピー
keybinding-desc-nbeditor-debug-copy-selection = リッチテキスト選択をコピー
keybinding-desc-nbeditor-log-state = エディタ状態をログ出力
keybinding-desc-nbeditor-edit-link = リンクを作成または編集
keybinding-desc-nbeditor-inline-code = インラインコードスタイルを切り替え
keybinding-desc-nbeditor-strikethrough = 取り消し線スタイルを切り替え
keybinding-desc-nbeditor-underline = 下線スタイルを切り替え
keybinding-desc-nbeditor-find = ノートブック内を検索
keybinding-desc-nbeditor-next-find-match = 次のマッチにフォーカス
keybinding-desc-nbeditor-previous-find-match = 前のマッチにフォーカス
keybinding-desc-nbeditor-toggle-regex-find = 正規表現検索を切り替え
keybinding-desc-nbeditor-toggle-case-sensitive-find = 大文字小文字を区別する検索を切り替え

# Pane group / undo close binding desc
keybinding-desc-get-started-terminal-session = ターミナルセッション
keybinding-desc-undo-close-reopen-session = 閉じたセッションを再オープン
keybinding-desc-right-panel-toggle-maximize-code-review = コードレビューパネルの最大化を切り替え

# Workspace sync inputs binding desc
keybinding-desc-workspace-disable-sync-inputs = すべてのペインの同期を停止
keybinding-desc-workspace-toggle-sync-inputs-tab = 現在のタブの全ペイン同期を切り替え
keybinding-desc-workspace-toggle-sync-inputs-all-tabs = すべてのタブの全ペイン同期を切り替え

# Workspace a11y / debug binding desc
keybinding-desc-workspace-a11y-concise = [a11y] アクセシビリティ通知を簡潔に設定
keybinding-desc-workspace-a11y-verbose = [a11y] アクセシビリティ通知を詳細に設定
keybinding-desc-workspace-copy-access-token = アクセストークンをクリップボードにコピー

# Env var collection binding desc
keybinding-desc-env-var-collection-close = 閉じる

# Auth / share modal binding desc
keybinding-desc-share-block-copy = コピー
keybinding-desc-auth-paste-token = 貼り付け
keybinding-desc-conversation-details-copy = コピー

# Terminal extras binding desc
keybinding-desc-terminal-show-history = 履歴を表示
keybinding-desc-terminal-ask-ai-selection = 選択について Warp AI に質問
keybinding-desc-terminal-ask-ai-last-block = 直前のブロックについて Warp AI に質問
keybinding-desc-terminal-ask-ai = Warp AI に質問
keybinding-desc-terminal-load-agent-conversation = エージェントモード会話を読み込み (クリップボードのデバッグリンクから)
keybinding-desc-terminal-toggle-session-recording = セッションの PTY 記録を切り替え

# Notebook editor extra
keybinding-desc-nbeditor-select-to-paragraph-start = 段落の先頭まで選択
keybinding-desc-nbeditor-select-to-paragraph-end = 段落の末尾まで選択

# Misc binding desc(收尾批次:常量/LazyLock/动态描述去硬编码)
keybinding-desc-save-file = ファイルを保存
keybinding-desc-new-agent-pane = 新規エージェントペイン
keybinding-desc-edit-code-diff = コード diff を編集
keybinding-desc-edit-requested-command = リクエスト中のコマンドを編集
keybinding-desc-set-input-mode-agent = 入力モードをエージェントモードに設定
keybinding-desc-set-input-mode-terminal = 入力モードをターミナルモードに設定
keybinding-desc-toggle-hide-cli-responses = CLI レスポンス非表示を切り替え
keybinding-desc-slash-command = スラッシュコマンド: { $name }
keybinding-desc-take-control-of-running-command = 実行中コマンドの制御を引き継ぐ

# --- Terminal zero-state block (welcome chips) ---
terminal-zero-state-title = 新規ターミナルセッション
terminal-zero-state-start-agent = 新規エージェント会話を開始
terminal-zero-state-cycle-history = 過去のコマンドと会話を巡回
terminal-zero-state-open-code-review = コードレビューを開く
terminal-zero-state-autodetect-prompts = ターミナルセッションでエージェントプロンプトを自動検出
terminal-zero-state-dismiss = 今後表示しない

# --- Rules page (ai/facts/view/rule.rs) ---
rules-description = ルールは構造化されたガイドラインを提供することでエージェントを強化し、一貫性の維持・ベストプラクティスの徹底・コードベースや広範なタスクなど特定ワークフローへの適応を支援します。
rules-search-placeholder = ルールを検索
rules-name-placeholder = 例: Rust ルール
rules-description-placeholder = 例: Rust では unwrap を使わない
rules-zero-state-global = ルールを追加すると、ここに表示されます。
rules-zero-state-project = プロジェクトの WARP.md ルールファイルを生成すると、ここに表示されます。
rules-disabled-banner-prefix = ルールは無効化されており、セッションのコンテキストとして使用されません。{" "}
rules-disabled-banner-link = いつでも再度有効化
rules-disabled-banner-suffix = {" "}できます。
rules-tab-global = グローバル
rules-tab-project = プロジェクト別
rules-add-button = 追加
rules-init-project-button = プロジェクトを初期化

# --- Agent view zero-state + message bar ---
agent-zero-state-title = 新規 Oz エージェント会話
agent-zero-state-title-cloud = 新規 Oz ローカルエージェント会話
agent-zero-state-description = 下にプロンプトを送信して新規会話を開始
agent-zero-state-description-with-location = `{ $location }` で新規会話を開始するには下にプロンプトを送信
agent-zero-state-switch-model = モデルを切り替え
agent-zero-state-go-back-to-terminal = ターミナルに戻る
agent-message-bar-for-help = ヘルプ
agent-message-bar-for-commands = コマンド
agent-message-bar-open-conversation = 会話を開く
agent-message-bar-for-code-review = コードレビュー
agent-message-bar-resume-conversation = で会話を再開
agent-message-bar-hide-plan = でプランを非表示
agent-message-bar-view-plans = でプラン一覧を表示
agent-message-bar-view-plan = でプランを表示
agent-message-bar-fork-continue = でフォークして続行
agent-message-bar-new-pane = {" "}新規ペイン
agent-message-bar-new-tab = {" "}新規タブ
agent-message-bar-current-pane = {" "}現在のペイン
agent-message-bar-hide-help = でヘルプを非表示
agent-message-bar-autodetected-shell-command-prefix = 自動検出されたシェルコマンド、{" "}
agent-message-bar-autodetected-shell-command = 自動検出されたシェルコマンド
agent-message-bar-override = {" "}で上書き
agent-message-bar-exit-shell-mode = でシェルモードを抜ける
agent-message-bar-again-stop-exit = もう一度で停止して抜ける
agent-message-bar-again-exit = もう一度で抜ける
agent-message-bar-again-start-new-conversation = もう一度で新規会話を開始
agent-shortcuts-input-shell-command = シェルコマンドを入力
agent-shortcuts-slash-commands = スラッシュコマンド
agent-shortcuts-file-paths-context = ファイルパスや他のコンテキスト添付
agent-shortcuts-open-code-review = コードレビューを開く
agent-shortcuts-toggle-conversation-list = 会話リストを切り替え
agent-shortcuts-search-continue-conversations = 会話を検索して続行
agent-shortcuts-start-new-conversation = 新規会話を開始
agent-shortcuts-toggle-auto-accept = 自動承認を切り替え
agent-shortcuts-pause-agent = エージェントを一時停止
agent-error-will-resume-when-network-restored = ネットワーク接続が復旧したら会話を再開します...
agent-error-attempting-resume-conversation = 会話の再開を試みています...

# --- ANCHOR-SUB-TOGGLE-PAIR (settings-toggle-pair) ---
toggle-setting-enable = { $suffix }を有効化
toggle-setting-disable = { $suffix }を無効化

toggle-suffix-ai = AI
toggle-suffix-active-ai = アクティブ AI
toggle-suffix-ai-input-autodetect-agent = エージェント入力でのターミナルコマンド自動検出
toggle-suffix-ai-input-autodetect-nld = 自然言語検出
toggle-suffix-nld-in-terminal = ターミナル入力でのエージェントプロンプト自動検出
toggle-suffix-next-command = 次コマンド
toggle-suffix-prompt-suggestions = プロンプトサジェスト
toggle-suffix-code-suggestions = コードサジェスト
toggle-suffix-nl-autosuggestions = 自然言語オートサジェスト
toggle-suffix-voice-input = 音声入力
toggle-suffix-codebase-index = コードベースインデックス
toggle-suffix-auto-indexing = 自動インデックス
toggle-suffix-compact-mode = コンパクトモード
toggle-suffix-themes-sync-os = テーマ: OS と同期
toggle-suffix-cursor-blink = カーソル点滅
toggle-suffix-jump-bottom-block = ブロック末尾ジャンプボタン
toggle-suffix-block-dividers = ブロック区切り線
toggle-suffix-dim-inactive-panes = 非アクティブペインを暗くする
toggle-suffix-tab-indicators = タブインジケーター
toggle-suffix-focus-follows-mouse = マウス追従フォーカス
toggle-suffix-zen-mode = 禅モード
toggle-suffix-vertical-tabs = 縦タブレイアウト
toggle-suffix-ligature-rendering = リガチャ描画
toggle-suffix-copy-on-select = ターミナル内での選択時コピー
toggle-suffix-linux-selection-clipboard = Linux 選択クリップボード
toggle-suffix-autocomplete-symbols = クォート・括弧・ブラケットの自動補完
toggle-suffix-restore-session = 起動時にウィンドウ・タブ・ペインを復元
toggle-suffix-left-option-meta = 左 Option キーを Meta として扱う
toggle-suffix-left-alt-meta = 左 Alt キーを Meta として扱う
toggle-suffix-right-option-meta = 右 Option キーを Meta として扱う
toggle-suffix-right-alt-meta = 右 Alt キーを Meta として扱う
toggle-suffix-scroll-reporting = スクロールレポート
toggle-suffix-completions-while-typing = 入力中の補完
toggle-suffix-command-corrections = コマンド訂正
toggle-suffix-error-underlining = エラー下線
toggle-suffix-syntax-highlighting = シンタックスハイライト
toggle-suffix-audible-bell = ターミナル音響ベル
toggle-suffix-autosuggestions = オートサジェスト
toggle-suffix-autosuggestion-keybinding-hint = オートサジェストキーバインドヒント
toggle-suffix-ssh-wrapper = Warp SSH ラッパー
toggle-suffix-link-tooltip = リンククリック時のツールチップ表示
toggle-suffix-quit-warning = 終了警告モーダル
toggle-suffix-alias-expansion = エイリアス展開
toggle-suffix-middle-click-paste = 中クリックペースト
toggle-suffix-code-as-default-editor = code を既定エディタにする
toggle-suffix-input-hint-text = 入力ヒントテキスト
toggle-suffix-vim-keybindings = Vim キーバインドでのコマンド編集
toggle-suffix-vim-clipboard = Vim 無名レジスタをシステムクリップボードに
toggle-suffix-vim-status-bar = Vim ステータスバー
toggle-suffix-focus-reporting = フォーカスレポート
toggle-suffix-smart-select = スマート選択
toggle-suffix-input-message-line = ターミナル入力メッセージ行
toggle-suffix-slash-commands-terminal = ターミナルモードでのスラッシュコマンド
toggle-suffix-integrated-gpu = 統合 GPU 描画 (低消費電力)
toggle-suffix-wayland = ウィンドウ管理に Wayland を使用
toggle-suffix-settings-sync = 設定同期
toggle-suffix-app-analytics = アプリアナリティクス
toggle-suffix-crash-reporting = クラッシュレポート
toggle-suffix-secret-redaction = シークレットマスキング
toggle-suffix-recording-mode = 記録モード
toggle-suffix-inband-generators = 新規セッションでのインバンドジェネレーター
toggle-suffix-debug-network = ネットワーク状態デバッグ
toggle-suffix-memory-stats = メモリ統計

# Set agent thinking display
agent-thinking-display-show-collapse = エージェント思考表示を設定: 表示して折りたたむ
agent-thinking-display-always-show = エージェント思考表示を設定: 常に表示
agent-thinking-display-never-show = エージェント思考表示を設定: 表示しない

# --- ANCHOR-SUB-EXTERNAL-EDITOR (settings-external-editor) ---
settings-external-editor-choose-default = ファイルリンクを開くエディタを選択
settings-external-editor-choose-code-panels = コードレビューパネル・プロジェクトエクスプローラー・グローバル検索からファイルを開くエディタを選択
settings-external-editor-choose-layout = Warp でファイルを開くレイアウトを選択
settings-external-editor-tabbed-header = ファイルを単一エディタペインにグループ化
settings-external-editor-tabbed-desc = この設定がオンの場合、同じタブで開いたファイルは自動的に単一エディタペインにグループ化されます。
settings-external-editor-prefer-markdown = Markdown ファイルを既定で Warp の Markdown ビューアで開く
settings-external-editor-layout-split-pane = ペイン分割
settings-external-editor-layout-new-tab = 新規タブ
settings-external-editor-default-app = 既定アプリ

# =============================================================================
# =============================================================================
# SECTION: context-menu (Owner: agent-context-menu)
# 鼠标右键弹出菜单。surface 前缀:menu-{block,input,ai-block,tab,pane,filetree,codeeditor}-*
# =============================================================================

# --- block 右键菜单(terminal/view.rs) ---
menu-block-copy = コピー
menu-block-copy-url = URL をコピー
menu-block-copy-path = パスをコピー
menu-block-show-in-finder = Finder で表示
menu-block-show-containing-folder = 含まれるフォルダを表示
menu-block-open-in-warp = Warp で開く
menu-block-open-in-editor = エディタで開く
menu-block-insert-into-input = 入力欄に挿入
menu-block-copy-command = コマンドをコピー
menu-block-copy-commands = コマンドをコピー
menu-block-find-within-block = ブロック内を検索
menu-block-find-within-blocks = ブロック内を検索
menu-block-scroll-to-top-of-block = ブロックの先頭へスクロール
menu-block-scroll-to-top-of-blocks = ブロックの先頭へスクロール
menu-block-scroll-to-bottom-of-block = ブロックの末尾へスクロール
menu-block-scroll-to-bottom-of-blocks = ブロックの末尾へスクロール
menu-block-save-as-workflow = ワークフローとして保存
menu-block-ask-warp-ai = Warp AI に質問
menu-block-copy-output = 出力をコピー
menu-block-copy-filtered-output = フィルタ済み出力をコピー
menu-block-toggle-block-filter = ブロックフィルタを切り替え
menu-block-toggle-bookmark = ブックマークを切り替え
menu-block-copy-prompt = プロンプトをコピー
menu-block-copy-right-prompt = 右プロンプトをコピー
menu-block-copy-working-directory = 作業ディレクトリをコピー
menu-block-copy-git-branch = git ブランチをコピー
menu-block-edit-prompt = プロンプトを編集
menu-block-edit-cli-agent-toolbelt = CLI エージェントツールベルトを編集
menu-block-edit-agent-toolbelt = エージェントツールベルトを編集
menu-block-split-pane-right = ペインを右に分割
menu-block-split-pane-left = ペインを左に分割
menu-block-split-pane-down = ペインを下に分割
menu-block-split-pane-up = ペインを上に分割
menu-block-close-pane = ペインを閉じる

# --- input 右键菜单(terminal/view.rs) ---
menu-input-cut = 切り取り
menu-input-copy = コピー
menu-input-paste = 貼り付け
menu-input-select-all = すべて選択
menu-input-command-search = コマンド検索
menu-input-ai-command-search = AI コマンド検索
menu-input-ask-warp-ai = Warp AI に質問
menu-input-save-as-workflow = ワークフローとして保存
menu-input-hide-hint-text = 入力ヒントを非表示
menu-input-show-hint-text = 入力ヒントを表示

# --- AI block overflow 菜单(terminal/view.rs) ---
menu-ai-block-copy = コピー
menu-ai-block-copy-prompt = プロンプトをコピー
menu-ai-block-copy-output-as-markdown = 出力を Markdown としてコピー
menu-ai-block-copy-url = URL をコピー
menu-ai-block-copy-path = パスをコピー
menu-ai-block-copy-command = コマンドをコピー
menu-ai-block-copy-git-branch = git ブランチをコピー
menu-ai-block-save-as-prompt = プロンプトとして保存
menu-ai-block-copy-conversation-text = 会話テキストをコピー
menu-ai-block-fork-from-here = ここからフォーク
menu-ai-block-rewind-to-before-here = ここの直前まで巻き戻し
menu-ai-block-fork-from-last-query = 直前のクエリからフォーク
menu-ai-block-fork-from-query = "{ $query }" からフォーク

# --- tab 右键菜单(tab.rs) ---
menu-tab-stop-sharing = 共有を停止
menu-tab-stop-sharing-all = すべての共有を停止
menu-tab-copy-link = リンクをコピー
menu-tab-rename = タブの名前を変更
menu-tab-reset-name = タブ名をリセット
menu-tab-move-down = タブを下へ移動
menu-tab-move-right = タブを右へ移動
menu-tab-move-up = タブを上へ移動
menu-tab-move-left = タブを左へ移動
menu-tab-close = タブを閉じる
menu-tab-close-other = 他のタブを閉じる
menu-tab-close-below = 下のタブを閉じる
menu-tab-close-right = 右のタブを閉じる
menu-tab-save-as-new-config = 新しい構成として保存
menu-tab-default-no-color = デフォルト (色なし)

# --- pane header 溢出菜单(terminal/view/pane_impl.rs) ---
menu-pane-copy-link = リンクをコピー
menu-pane-stop-sharing-session = セッション共有を停止
menu-pane-open-on-desktop = デスクトップで開く

# --- 文件树右键菜单(code/file_tree/view.rs) ---
menu-filetree-open-in-new-pane = 新しいペインで開く
menu-filetree-open-in-new-tab = 新しいタブで開く
menu-filetree-open-file = ファイルを開く
menu-filetree-new-file = 新規ファイル
menu-filetree-cd-to-directory = ディレクトリへ cd
menu-filetree-reveal-finder = Finder で表示
menu-filetree-reveal-explorer = エクスプローラーで表示
menu-filetree-reveal-file-manager = ファイルマネージャーで表示
menu-filetree-rename = 名前変更
menu-filetree-delete = 削除
menu-filetree-attach-as-context = コンテキストとして添付
menu-filetree-copy-path = パスをコピー
menu-filetree-copy-relative-path = 相対パスをコピー

# --- 代码编辑器右键菜单(code/local_code_editor.rs) ---
menu-codeeditor-go-to-definition = 定義へ移動
menu-codeeditor-find-references = 参照を検索

# --- 共享标签:附加为 agent 上下文(blocklist/view_util.rs) ---
menu-attach-as-agent-context = エージェントコンテキストとして添付

# --- ANCHOR-SUB-SLASH-COMMANDS (agent-slash-commands) ---
# Slash command palette descriptions and argument hints
# (app/src/search/slash_command_menu/static_commands/commands.rs)
slash-cmd-agent-desc = 新しい会話を開始
slash-cmd-add-mcp-desc = 新しい MCP サーバーを追加
slash-cmd-pr-comments-desc = GitHub PR レビューコメントを取得
slash-cmd-create-environment-desc = ガイドに従って Oz 環境 (Docker イメージ + リポジトリ) を作成
slash-cmd-create-environment-hint = <任意のリポジトリパスまたは GitHub URL>
slash-cmd-docker-sandbox-desc = 新しい Docker サンドボックスのターミナルセッションを作成
slash-cmd-create-new-project-desc = Oz と一緒に新しいコーディングプロジェクトを作成
slash-cmd-create-new-project-hint = <作りたいものを記述>
slash-cmd-open-skill-desc = Warp 内蔵エディタでスキルの Markdown ファイルを開く
slash-cmd-skills-desc = スキルを呼び出す
slash-cmd-add-prompt-desc = 新しいエージェントプロンプトを追加
slash-cmd-add-rule-desc = エージェントの新しいグローバルルールを追加
slash-cmd-open-file-desc = Warp のコードエディタでファイルを開く
slash-cmd-open-file-hint = <path/to/file[:line[:col]]> または "@" で検索
slash-cmd-rename-tab-desc = 現在のタブの名前を変更
slash-cmd-rename-tab-hint = <タブ名>
slash-cmd-fork-desc = 現在の会話を新しいペインまたはタブにフォーク
slash-cmd-fork-hint = <フォーク後の会話に送る任意のプロンプト>
slash-cmd-open-code-review-desc = コードレビューを開く
slash-cmd-init-desc = AGENTS.md ファイルを生成または更新
slash-cmd-open-project-rules-desc = プロジェクトルールファイル (AGENTS.md) を開く
slash-cmd-open-mcp-servers-desc = MCP サーバーを開く
slash-cmd-open-settings-file-desc = 設定ファイル (TOML) を開く
slash-cmd-changelog-desc = 最新の changelog を開く
slash-cmd-open-repo-desc = 別のインデックス済みリポジトリに切り替え
slash-cmd-open-rules-desc = グローバルおよびプロジェクトのルールをすべて表示
slash-cmd-new-desc = 新しい会話を開始 (/agent のエイリアス)
slash-cmd-model-desc = ベースエージェントモデルを切り替え
slash-cmd-profile-desc = アクティブな実行プロファイルを切り替え
slash-cmd-plan-desc = エージェントに調査させてタスクの計画を作成
slash-cmd-plan-hint = <タスクを記述>
slash-cmd-orchestrate-desc = タスクをサブタスクに分解し複数エージェントで並列実行
slash-cmd-orchestrate-hint = <タスクを記述>
slash-cmd-compact-desc = 会話履歴を要約してコンテキストを解放
slash-cmd-compact-hint = <任意のカスタム要約指示>
slash-cmd-compact-and-desc = 会話を圧縮した後に追加プロンプトを送信
slash-cmd-compact-and-hint = <圧縮後に送信するプロンプト>
slash-cmd-queue-desc = エージェントの応答完了後に送信するプロンプトをキュー
slash-cmd-queue-hint = <エージェント完了時に送信するプロンプト>
slash-cmd-fork-and-compact-desc = 現在の会話をフォークしフォーク先で圧縮
slash-cmd-fork-and-compact-hint = <圧縮後に送信する任意のプロンプト>
slash-cmd-fork-from-desc = 特定のクエリから会話をフォーク
slash-cmd-remote-control-desc = このセッションのリモート操作を開始
slash-cmd-conversations-desc = 会話履歴を開く
slash-cmd-prompts-desc = 保存済みプロンプトを検索
slash-cmd-rewind-desc = 会話の前の時点まで巻き戻し
slash-cmd-export-to-clipboard-desc = 現在の会話を Markdown 形式でクリップボードにエクスポート
slash-cmd-export-to-file-desc = 現在の会話を Markdown ファイルにエクスポート
slash-cmd-export-to-file-hint = <任意のファイル名>

# --- ANCHOR-SUB-PROMPT-TIPS ---
# Prompt editor modal (app/src/prompt/editor_modal.rs)
prompt-editor-title = プロンプトを編集
prompt-editor-warp-prompt-section = Warp ターミナルプロンプト
prompt-editor-shell-prompt-section = シェルプロンプト (PS1)
prompt-editor-restore-default = デフォルトに戻す
prompt-editor-same-line-prompt = 同一行プロンプト
prompt-editor-separator = 区切り文字
prompt-editor-cancel = キャンセル
prompt-editor-save-changes = 変更を保存

# Welcome tips (app/src/tips/tip_view.rs)
welcome-tips-command-palette-title = コマンドパレット
welcome-tips-command-palette-description = キーボードから手を離さずに Warp の機能をすべて簡単に発見できます。
welcome-tips-split-pane-title = ペイン分割
welcome-tips-split-pane-description = タブを複数のペインに分割して理想的なレイアウトを作成。
welcome-tips-history-search-title = 履歴検索
welcome-tips-history-search-description = 過去に実行したコマンドを検索・編集・再実行。
welcome-tips-ai-command-search-title = AI コマンド検索
welcome-tips-ai-command-search-description = 自然言語からシェルコマンドを生成。
welcome-tips-theme-picker-title = テーマピッカー
welcome-tips-theme-picker-description = 組み込みテーマから選んで Warp を自分好みに。または自作も可能。
welcome-tips-shortcut-label = ショートカット
welcome-tips-skip = ようこそチップをスキップ
welcome-tips-complete-title = 完了!
welcome-tips-complete-description = ようこそチップの完了お疲れさまでした!
welcome-tips-close = ようこそチップを閉じる

# --- ANCHOR-SUB-SMALL-DIALOGS ---
# Rewind confirmation dialog (app/src/workspace/rewind_confirmation_dialog.rs)
rewind-dialog-title = 巻き戻し
rewind-dialog-body = 巻き戻してもよろしいですか? コードと会話をこの時点より前の状態に戻し、エージェントが現在実行中のコマンドをすべてキャンセルします。元の会話のコピーは会話履歴に保存されます。
rewind-dialog-info = 巻き戻しは手動またはシェルコマンドで編集されたファイルには影響しません。
rewind-dialog-cancel = キャンセル
rewind-dialog-confirm = 巻き戻し

# --- ANCHOR-SUB-SEARCH-PALETTES ---
# Search palettes (app/src/search/command_palette/view.rs, app/src/search/welcome_palette/view.rs)
command-palette-search-placeholder = コマンドを検索
command-palette-no-results = 該当する結果がありません
command-palette-toast-cannot-switch-conversations = エージェントがコマンドを監視中は会話を切り替えできません。
command-palette-toast-cannot-start-new-conversation = エージェントがコマンドを監視中は新しい会話を開始できません。
command-palette-zero-state-recent = 最近
command-palette-zero-state-suggested = 候補
welcome-palette-search-placeholder = コーディング、ビルド、または何でも検索…
welcome-palette-no-results = 該当する結果がありません
search-filter-placeholder-history = 履歴を検索
search-filter-placeholder-workflows = ワークフローを検索
search-filter-placeholder-agent-mode-workflows = プロンプトを検索
search-filter-placeholder-notebooks = ノートブックを検索
search-filter-placeholder-plans = プランを検索
search-filter-placeholder-natural-language = 例: ファイル内の文字列を置換
search-filter-placeholder-actions = アクションを検索
search-filter-placeholder-sessions = セッションを検索
search-filter-placeholder-conversations = 会話を検索
search-filter-placeholder-historical-conversations = 過去の会話を検索
search-filter-placeholder-launch-configurations = 起動構成を検索
search-filter-placeholder-drive = Drive 内のオブジェクトを検索
search-filter-placeholder-environment-variables = 環境変数を検索
search-filter-placeholder-prompt-history = プロンプト履歴を検索
search-filter-placeholder-files = ファイルを検索
search-filter-placeholder-commands = コマンドを検索
search-filter-placeholder-blocks = ブロックを検索
search-filter-placeholder-code = コードシンボルを検索
search-filter-placeholder-rules = AI ルールを検索
search-filter-placeholder-repos = コードリポジトリを検索
search-filter-placeholder-diff-sets = diff セットを検索
search-filter-placeholder-static-slash-commands = 静的スラッシュコマンドを検索
search-filter-placeholder-skills = スキルを検索
search-filter-placeholder-base-models = ベースモデルを検索
search-filter-placeholder-full-terminal-use-models = フルターミナル用モデルを検索
search-filter-placeholder-current-directory-conversations = 現在のディレクトリの会話を検索
search-filter-display-history = 履歴
search-filter-display-workflows = ワークフロー
search-filter-display-agent-mode-workflows = プロンプト
search-filter-display-notebooks = ノートブック
search-filter-display-plans = プラン
search-filter-display-natural-language = AI コマンド候補
search-filter-display-actions = アクション
search-filter-display-sessions = セッション
search-filter-display-conversations = 会話
search-filter-display-launch-configurations = 起動構成
search-filter-display-drive = Warp Drive
search-filter-display-environment-variables = 環境変数
search-filter-display-prompt-history = プロンプト履歴
search-filter-display-files = ファイル
search-filter-display-commands = コマンド
search-filter-display-blocks = ブロック
search-filter-display-code = コード
search-filter-display-rules = ルール
search-filter-display-repos = リポジトリ
search-filter-display-diff-sets = diff セット
search-filter-display-static-slash-commands = スラッシュコマンド
search-filter-display-historical-conversations = 過去の会話
search-filter-display-skills = スキル
search-filter-display-base-models = ベースモデル
search-filter-display-full-terminal-use-models = フルターミナル用モデル
search-filter-display-current-directory-conversations = 現在のディレクトリの会話
search-results-menu-no-results = 該当する結果がありません
search-results-menu-prompts-title = プロンプト
ai-context-diffset-uncommitted-changes = 未コミットの変更
ai-context-diffset-changes-vs-main-branch = main ブランチとの差分
ai-context-diffset-changes-vs-branch = { $branch } との差分
ai-context-diffset-uncommitted-changes-description = 作業ディレクトリ内のすべての未コミットの変更
ai-context-diffset-changes-vs-main-branch-description = main ブランチと比較したすべての変更
ai-context-diffset-changes-vs-branch-description = { $branch } と比較したすべての変更
ai-context-code-search-failed = コード検索に失敗しました
ai-context-files-directory-accessibility-label = ディレクトリ: { $path }
ai-context-files-file-accessibility-label = ファイル: { $path }
ai-context-blocks-just-now = たった今
ai-context-blocks-minutes-ago = { $count ->
        [one] 1 分前
       *[other] { $count } 分前
    }
ai-context-blocks-hours-ago = { $count ->
        [one] 1 時間前
       *[other] { $count } 時間前
    }
ai-context-blocks-days-ago = { $count ->
        [one] 1 日前
       *[other] { $count } 日前
    }
ai-context-blocks-no-output = 出力なし
ai-context-blocks-accessibility-label = ブロック: { $command }

# --- ANCHOR-SUB-DRIVE-NAMING-IMPORT ---
# Drive naming dialog (app/src/drive/cloud_object_naming_dialog.rs)
drive-naming-notebook-name = ノートブック名
drive-naming-folder-name = フォルダ名
drive-naming-collection-name = コレクション名
drive-naming-create = 作成
drive-naming-cancel = キャンセル
drive-naming-rename = 名前変更

# Drive import modal (app/src/drive/import/modal.rs, app/src/drive/import/modal_body.rs)
drive-import-title = インポート
drive-import-close = 閉じる
drive-import-cancel = キャンセル
drive-import-preparing = 準備中…
drive-import-choose-files = ファイルを選択…
drive-import-learn-file-support = ファイルサポートとフォーマットについて
drive-import-file-upload-error = サーバーへのファイルアップロードに失敗しました
drive-import-folder-upload-error = サーバーへのフォルダアップロードに失敗しました

# Drive main panel and workflow editor (app/src/drive/index.rs, app/src/drive/workflows/*)
drive-title = Drive
drive-environment-variables = 環境変数
drive-folder = フォルダ
drive-notebook = ノートブック
drive-workflow = ワークフロー
drive-prompt = プロンプト
drive-import = インポート
drive-remove = 削除
drive-new-folder = 新規フォルダ
drive-new-notebook = 新規ノートブック
drive-new-workflow = 新規ワークフロー
drive-new-prompt = 新規プロンプト
drive-new-environment-variables = 新規環境変数
drive-offline-banner = オフラインです。一部のファイルは読み取り専用になります。
drive-sort-by = 並び替え
drive-retry-sync = 同期を再試行
drive-empty-trash = ゴミ箱を空にする
drive-trash-section-title = ゴミ箱
drive-trash-title = ゴミ箱
drive-trash-deletion-warning = ゴミ箱内のアイテムは 30 日後に完全に削除されます。
drive-team-space-zero-state = 個人のワークフローまたはノートブックをここにドラッグまたは移動してチームと共有しましょう。
drive-sign-up-storage-limit = 無料登録するとストレージ上限が拡大し、より多くの機能を利用できます。
drive-sign-up = 登録
drive-copy-link = リンクをコピー
drive-collapse-all = すべて折りたたむ
drive-revert-to-server = サーバーの状態に戻す
drive-attach-to-active-session = アクティブセッションに添付
drive-copy-prompt = プロンプトをコピー
drive-copy-workflow-text = ワークフローテキストをコピー
drive-copy-id = ID をコピー
drive-copy-variables = 変数をコピー
drive-load-in-subshell = サブシェルで読み込み
drive-delete-forever = 完全に削除
drive-rename = 名前変更
drive-retry = 再試行
drive-move-to-space = { $space } へ移動
drive-open-on-desktop = デスクトップで開く
drive-duplicate = 複製
drive-export = エクスポート
drive-trash-menu = ゴミ箱
drive-open = 開く
drive-edit = 編集
drive-restore = 復元
drive-compare-plans = プランを比較
drive-manage-billing = 請求情報を管理
drive-object-type-notebook-plural = ノートブック
drive-object-type-workflow-plural = ワークフロー
drive-object-type-folder-plural = フォルダ
drive-object-type-env-var-collection-plural = 環境変数コレクション
drive-object-type-object-plural = オブジェクト
drive-object-type-notebooks = ノートブック
drive-object-type-workflows = ワークフロー
drive-object-type-environment-variables = 環境変数
drive-object-type-folders = フォルダ
drive-object-type-agent-workflows = エージェントワークフロー
drive-object-type-ai-fact = AI ファクト
drive-object-type-rules = ルール
drive-object-type-mcp-server = MCP サーバー
drive-object-type-mcp-servers = MCP サーバー
drive-shared-object-limit-hit-banner-prefix = ローカルの { $object_type } 上限に達しました。
drive-shared-object-limit-hit-banner = ローカルの { $object_type } 上限に達しました。
drive-payment-issue-banner-prefix = サブスクリプションの支払いに問題があるため、共有オブジェクトが制限されています。
drive-payment-issue-banner-admin = サブスクリプションの支払いに問題があるため、共有オブジェクトが制限されています。アクセスを復旧するには支払い情報を更新してください。
drive-payment-issue-banner-admin-enterprise = サブスクリプションの支払いに問題があるため、共有オブジェクトが制限されています。アクセスを復旧するには support@warp.dev までご連絡ください。
drive-payment-issue-banner-nonadmin = サブスクリプションの支払いに問題があるため、共有オブジェクトが制限されています。チーム管理者にご連絡ください。
drive-empty-trash-title = ゴミ箱を空にしますか?
drive-empty-trash-body = この操作は取り消せません。
drive-empty-trash-confirm = はい、ゴミ箱を空にする
drive-empty-trash-cancel = キャンセル
workflow-title-placeholder = 無題のワークフロー
workflow-description-placeholder = 説明を追加
workflow-title-input-placeholder = タイトルを追加
workflow-description-input-placeholder = 説明を追加
workflow-new-argument = 新しい引数
workflow-arguments-label = 引数
workflow-argument-description-placeholder = 説明
workflow-argument-value-placeholder = 値 (任意)
workflow-default-value-placeholder = デフォルト値 (任意)
workflow-agent-mode-query-placeholder = ここにプロンプトを入力してください...(例:「日付でオブジェクトの配列をソートする関数を作成」「この React コンポーネントのデバッグを手伝って」)。
workflow-save = ワークフローを保存
workflow-unsaved-changes = 未保存の変更があります。
workflow-keep-editing = 編集を続ける
workflow-discard-changes = 変更を破棄
workflow-ai-assist-autofill = 自動入力
workflow-ai-assist-loading = 読み込み中
workflow-ai-assist-tooltip = Warp AI でタイトル・説明・パラメータを生成
workflow-tooltip-restore-from-trash = ワークフローをゴミ箱から復元
workflow-ai-assist-error-byop-required = 自動入力には BYOP モデルが必要です。設定 → AI でプロバイダとモデルを設定してください。
workflow-ai-assist-error-bad-command = メタデータの生成に失敗しました。別のコマンドで再度お試しください。
workflow-ai-assist-error-generic = 問題が発生しました。再度お試しください。
workflow-ai-assist-error-rate-limited = AI クレジットが不足しているようです。しばらくしてから再度お試しください。
workflow-enum-new = 新規
workflow-alias-name-placeholder = エイリアス名
workflow-add-argument-tooltip = ワークフロー引数を追加

# --- ANCHOR-SUB-SETTINGS-PRIVACY-ADD-REGEX ---
# Privacy settings add regex modal (app/src/settings_view/privacy/add_regex_modal.rs)
settings-privacy-add-regex-name-placeholder = 例: "Google API Key"
settings-privacy-add-regex-name-label = 名前 (任意)
settings-privacy-add-regex-pattern-label = 正規表現パターン
settings-privacy-add-regex-invalid = 無効な正規表現
settings-privacy-add-regex-cancel = キャンセル

# Workspace panels (app/src/workspace/view/*)
workspace-conversation-list-search = 検索
workspace-conversation-list-active = アクティブ
workspace-conversation-list-past = 過去
workspace-conversation-list-view-all = すべて表示
workspace-conversation-list-show-less = 折りたたむ
workspace-conversation-list-empty-title = 会話がまだありません
workspace-conversation-list-empty-description = ローカル/アンビエントエージェントとのアクティブな会話および過去の会話がここに表示されます。
workspace-conversation-list-new-conversation = 新しい会話
workspace-conversation-list-no-matching = 一致する会話がありません
workspace-conversation-list-delete = 削除
workspace-conversation-list-delete-in-progress-error = 進行中の会話は削除できません。
workspace-conversation-list-delete-ambient-tooltip = アンビエントエージェントの会話は削除できません
workspace-conversation-list-fork-new-pane = 新しいペインで分岐
workspace-conversation-list-fork-new-tab = 新しいタブで分岐
workspace-conversation-list-fallback-title = 会話
workspace-left-panel-project-explorer = プロジェクトエクスプローラ
workspace-left-panel-global-search = グローバル検索
workspace-left-panel-warp-drive = Warp Drive
workspace-left-panel-agent-conversations = エージェント会話
workspace-left-panel-ssh-manager = SSH マネージャー
workspace-left-panel-ssh-manager-placeholder = SSH マネージャー — 近日公開
workspace-left-panel-ssh-manager-detail-empty = サーバーを選択して詳細を表示します。
workspace-left-panel-ssh-manager-detail-host = ホスト
workspace-left-panel-ssh-manager-detail-port = ポート
workspace-left-panel-ssh-manager-detail-user = ユーザー
workspace-left-panel-ssh-manager-detail-auth = 認証
workspace-left-panel-ssh-manager-detail-key-path = 鍵のパス
workspace-left-panel-ssh-manager-auth-password = パスワード
workspace-left-panel-ssh-manager-auth-key = 秘密鍵
workspace-left-panel-ssh-manager-menu-new-folder = 新しいフォルダ
workspace-left-panel-ssh-manager-menu-new-server = 新しい SSH サーバー
workspace-left-panel-ssh-manager-menu-edit = 編集
workspace-left-panel-ssh-manager-menu-connect = 接続
workspace-left-panel-ssh-manager-menu-delete = 削除
workspace-left-panel-ssh-manager-pane-hint = フィールドの編集と「接続」は次のイテレーションで提供されます。現状このペインは保存済み設定を表示するだけです。SQLite ストアか、まもなく登場するエディタで調整してください。
workspace-left-panel-ssh-manager-pane-folder-body = フォルダ。フォルダ内のサーバーを選択すると詳細が表示されます。フォルダを右クリックすると作成/削除アクションが利用できます。
workspace-left-panel-ssh-manager-server-missing = サーバーが見つかりません。別のウィンドウから削除された可能性があります。
workspace-left-panel-ssh-manager-field-name = 名前
workspace-left-panel-ssh-manager-passphrase = パスフレーズ
workspace-left-panel-ssh-manager-save = 保存
workspace-left-panel-ssh-manager-status-saved = 保存しました。
workspace-left-panel-ssh-manager-error-name-required = 名前は空にできません。
workspace-left-panel-ssh-manager-error-port-invalid = ポートは 1〜65535 の数値を指定してください。
workspace-left-panel-ssh-manager-error-host-required = ホストは空にできません。
workspace-left-panel-ssh-manager-connect = 接続
search-filter-placeholder-ssh-servers = SSH サーバーを検索...
search-filter-display-ssh-servers = SSH サーバー
workspace-left-panel-ssh-manager-menu-rename = 名前変更
workspace-left-panel-ssh-manager-tree-empty = SSH サーバーがまだありません。📁 でフォルダ追加、+ でサーバー追加。
workspace-left-panel-close-panel = パネルを閉じる
workspace-tabs-panel-tooltip = タブパネル
workspace-tools-panel-tooltip = ツールパネル
workspace-agent-management-panel-tooltip = エージェント管理パネル
workspace-code-review-panel-tooltip = コードレビューパネル
workspace-notifications-tooltip = 通知
workspace-new-tab-tooltip = 新しいタブ
workspace-tab-configs-tooltip = タブ設定
workspace-offline-tooltip = オフラインでは一部機能が利用できません
workspace-right-panel-open-repository = リポジトリを開く
workspace-right-panel-open-repository-tooltip = リポジトリに移動してコーディング用に初期化
workspace-right-panel-close-panel = パネルを閉じる
workspace-right-panel-code-review = コードレビュー
workspace-right-panel-minimize = 最小化
workspace-right-panel-maximize = 最大化
terminal-pane-new-cloud-agent-title = 新しいエージェント
terminal-pane-new-agent-conversation-title = 新しいエージェント会話
vertical-tabs-no-tabs-open = 開いているタブはありません
vertical-tabs-untitled-tab = 無題のタブ
vertical-tabs-view-options-tooltip = 表示オプション
vertical-tabs-new-session = 新しいセッション
vertical-tabs-terminal-kind-oz = Oz
vertical-tabs-pane-kind-terminal = ターミナル
vertical-tabs-pane-kind-code = コード
vertical-tabs-pane-kind-code-diff = コード差分
vertical-tabs-pane-kind-file = ファイル
vertical-tabs-pane-kind-notebook = ノートブック
vertical-tabs-pane-kind-workflow = ワークフロー
vertical-tabs-pane-kind-environment-variables = 環境変数
vertical-tabs-pane-kind-environments = 環境
vertical-tabs-pane-kind-rules = ルール
vertical-tabs-pane-kind-plan = プラン
vertical-tabs-pane-kind-execution-profile = 実行プロファイル
vertical-tabs-pane-kind-other = その他
vertical-tabs-setting-view-as = 表示形式
vertical-tabs-setting-panes = ペイン
vertical-tabs-setting-tabs = タブ
vertical-tabs-setting-tab-item = タブアイテム
vertical-tabs-setting-focused-session = フォーカス中のセッション
vertical-tabs-setting-summary = サマリー
vertical-tabs-setting-density = 密度
vertical-tabs-setting-pane-title-as = ペインタイトル形式
vertical-tabs-setting-command-conversation = コマンド/会話
vertical-tabs-setting-working-directory = 作業ディレクトリ
vertical-tabs-setting-branch = ブランチ
vertical-tabs-setting-additional-metadata = 追加メタデータ
vertical-tabs-setting-show = 表示
vertical-tabs-setting-pr-link-requires-gh = GitHub CLI のインストールと認証が必要です
vertical-tabs-setting-pr-link = PR リンク
vertical-tabs-setting-diff-stats = 差分統計
vertical-tabs-setting-show-details-on-hover = ホバー時に詳細を表示
workspace-right-panel-unknown = 不明
global-search-placeholder = ファイル内を検索
global-search-toggle-case-sensitivity = 大文字小文字を区別を切替
global-search-toggle-regex = 正規表現を切替
global-search-label = 検索
global-search-no-results-gitignore = 結果が見つかりませんでした。gitignore ファイルを確認してください。
global-search-result-count-one = { $files } { $files ->
        [one] 件のファイル
       *[other] 件のファイル
    }内に 1 件
global-search-result-count-many = { $files } { $files ->
        [one] 件のファイル
       *[other] 件のファイル
    }内に { $n } 件
global-search-subset-warning = 結果はすべての一致のうちの一部のみを含んでいます。検索条件をより具体的にして絞り込んでください。
global-search-title = グローバル検索
global-search-description = 現在のディレクトリ全体のファイル内を検索します。
global-search-unavailable-title = グローバル検索を利用できません
global-search-unavailable-description = グローバル検索にはローカルワークスペースへのアクセスが必要です。新しいセッションを開くか、アクティブなセッションに移動してください。
global-search-remote-description = グローバル検索にはローカルワークスペースへのアクセスが必要で、リモートセッションでは未対応です
global-search-unsupported-session-description = グローバル検索は現在 Git Bash や WSL では動作しません。
global-search-failed = グローバル検索に失敗しました。

# Wasm NUX dialog (app/src/wasm_nux_dialog.rs)
wasm-nux-open-desktop-title = Warp デスクトップで開きますか?
wasm-nux-open-desktop-detail = 今後のリンクは自動的にデスクトップで開きます。
wasm-nux-open-desktop-confirm = Warp で開く
wasm-nux-download-title = Warp デスクトップをダウンロードしますか?
wasm-nux-download-description = Warp は AI と開発チームのナレッジを内蔵したインテリジェントなターミナルです。
wasm-nux-learn-more = 詳しく見る
wasm-nux-download-confirm = ダウンロード
wasm-nux-object-kind-drive-objects = Warp Drive オブジェクト
wasm-nux-object-kind-warp-links = Warp リンク
wasm-nux-always-open-on-web-title = { $object_kind } を常に Web で開きますか?
wasm-nux-always-open-on-web-detail = この設定は設定画面でいつでも変更できます。
wasm-nux-yes = はい

# Auth override warning (app/src/auth/auth_override_warning_body.rs)
auth-override-warning-title = 新しいログインを検出しました
auth-override-warning-confirm-title = 個人の Warp Drive オブジェクトと環境設定を削除しますか?
auth-override-warning-description = Web ブラウザから Warp アカウントにログインしたようです。続行すると、この匿名セッションの個人 Warp Drive オブジェクトおよび環境設定はすべて完全に削除されます。
auth-override-warning-cannot-undo = この操作は取り消せません。
auth-override-warning-export = データをエクスポート
auth-override-warning-export-description =  して後でインポートできます。
auth-override-warning-cancel = キャンセル
auth-override-warning-continue = 続行
auth-override-warning-accessibility-help = Warp が Web ブラウザからの新しいログインを検出しました。ログインせずに Warp の使用を続けるには Esc を押してキャンセルしてください。

# Auth SSO link/login failures/paste token/logout/offline/privacy
auth-needs-sso-link-button = SSO をリンク
auth-needs-sso-link-title = 組織がアカウントの SSO を有効化しています
auth-needs-sso-link-detail = 下のボタンをクリックして Warp アカウントを SSO プロバイダにリンクしてください。
auth-login-failure-troubleshooting-prefix =  初めてではありませんか? こちらの
auth-login-failure-troubleshooting-link =  トラブルシューティングドキュメント
auth-login-failure-troubleshooting-suffix = をご覧ください。
auth-login-failure-invalid-token = 無効な認証トークンがモーダルに入力されました。
auth-login-failure-copy-token-manually = ログインに失敗しました。認証 Web ページから手動で認証トークンをコピーしモーダルに貼り付けてみてください。
auth-login-failure-login-request = ログインリクエストに失敗しました。
auth-login-failure-signup-request = サインアップリクエストに失敗しました。
auth-login-failure-wrong-redirect-url = 貼り付けられたリダイレクト URL はこのアプリ由来ではありません。下のボタンをクリックして再度お試しください。
auth-paste-token-placeholder = 認証トークンを入力
auth-paste-token-title = 認証トークンを下に貼り付けてください
auth-paste-token-detail = ブラウザから認証トークンを貼り付けてログインを完了してください。
auth-paste-token-cancel = キャンセル
auth-paste-token-continue = 続行
auth-offline-first-use-description = 現在オフラインです。Warp を初めて使用するにはインターネット接続が必要です。
auth-offline-first-use-learn-more = 詳しく見る
auth-offline-overlay-title = Warp をオフラインで使用
auth-offline-overlay-paragraph-1 = OpenWarp のローカル機能はオフラインで動作します。
auth-offline-overlay-paragraph-2 = BYOP AI 機能を使う場合のみ、選択したプロバイダーへの接続が必要です。
auth-offline-overlay-paragraph-3 = OpenWarp は匿名のローカルユーザー ID で動作し、外部オブジェクトや利用量計測のためにインターネット接続を要求しません。
auth-offline-overlay-dismiss = 閉じる
auth-privacy-settings-title = プライバシー設定
auth-privacy-settings-done = 完了
auth-privacy-settings-help-improve = Warp の改善に協力する
auth-privacy-settings-help-improve-description = ハイレベルな機能利用データは Warp プロダクトチームのロードマップ優先順位付けに役立ちます。
auth-privacy-settings-learn-more = 詳しく見る
auth-privacy-settings-send-crash-reports = クラッシュレポートを送信
auth-privacy-settings-crash-reports-description = クラッシュレポートは Warp エンジニアリングチームが安定性を理解しパフォーマンスを改善するのに役立ちます。
auth-logout-confirm = はい、ログアウトする
auth-logout-show-running-processes = 実行中のプロセスを表示
auth-logout-cancel = キャンセル
auth-logout-title = ログアウトしますか?
auth-logout-running-processes-warning = { $count } { $count ->
        [one] 件のプロセス
       *[other] 件のプロセス
    }が実行中です。
auth-logout-shared-sessions-warning = 共有セッションが { $count } { $count ->
        [one] 件
       *[other] 件
    }あります。
auth-logout-unsynced-drive-objects-warning = 未同期の Warp Drive オブジェクトが { $count } { $count ->
        [one] 件
       *[other] 件
    }あります。ログアウトすると、{ $count ->
        [one] そのオブジェクト
       *[other] それらのオブジェクト
    }は失われます。
auth-logout-unsaved-files-warning = 未保存のファイルが { $count } { $count ->
        [one] 件
       *[other] 件
    }あります。ログアウトすると、{ $count ->
        [one] そのファイル
       *[other] それらのファイル
    }は失われます。

# CLI agent plugin instructions
cli-agent-plugin-run-on-remote = これらのコマンドは必ずリモートマシン上で実行してください。
cli-agent-plugin-codex-install-title = Codex の Warp 通知を有効化
cli-agent-plugin-codex-install-subtitle = Codex を最新版に更新し、フォーカス時通知を有効にすることで Warp が作業中に通知を表示できるようにします。
cli-agent-plugin-codex-update-step = Codex を最新版に更新します。
cli-agent-plugin-codex-notification-step = Codex の設定で通知条件を "always" に設定します。~/.codex/config.toml を開くか作成して以下を追加してください:
cli-agent-plugin-codex-restart-note = 変更を反映するため Codex を再起動します。
cli-agent-plugin-deepseek-install-title = DeepSeek の Warp 通知を有効化
cli-agent-plugin-deepseek-install-subtitle = DeepSeek 設定ファイル (~/.deepseek/config.toml) に以下を追加してターン完了通知を有効にします。
cli-agent-plugin-deepseek-notification-step = ~/.deepseek/config.toml で通知条件を "always" に設定します:
cli-agent-plugin-deepseek-restart-note = 変更を反映するため DeepSeek を再起動します。
cli-agent-plugin-claude-install-title = Claude Code 用 Warp プラグインをインストール
cli-agent-plugin-claude-install-subtitle = まずマシンに jq がインストールされていることを確認し、続けて以下のコマンドを実行してください。
cli-agent-plugin-claude-add-marketplace-step = Warp プラグインのマーケットプレイスリポジトリを追加
cli-agent-plugin-install-warp-plugin-step = Warp プラグインをインストール
cli-agent-plugin-claude-restart-note = プラグインを有効化するため Claude Code を再起動します。
cli-agent-plugin-claude-known-issues-note = Claude Code のプラグインシステムには既知の問題があります。手順 1 後にプラグインが見つからない場合は、~/.claude/settings.json に "extraKnownMarketplaces" エントリを手動で追加してみてください。
cli-agent-plugin-claude-update-title = Claude Code 用 Warp プラグインを更新
cli-agent-plugin-run-following-commands = 以下のコマンドを実行してください。
cli-agent-plugin-remove-existing-marketplace-step = 既存のマーケットプレイスを削除 (存在する場合)
cli-agent-plugin-readd-marketplace-step = マーケットプレイスを再追加
cli-agent-plugin-install-latest-version-step = 最新版のプラグインをインストール
cli-agent-plugin-claude-restart-update-note = 更新を有効化するため Claude Code を再起動します。
cli-agent-plugin-gemini-install-title = Gemini CLI 用 Warp プラグインをインストール
cli-agent-plugin-gemini-run-command-restart = 以下のコマンドを実行し、Gemini CLI を再起動してください。
cli-agent-plugin-install-warp-extension-step = Warp 拡張機能をインストール
cli-agent-plugin-gemini-restart-note = プラグインを有効化するため Gemini CLI を再起動します。
cli-agent-plugin-gemini-update-title = Gemini CLI 用 Warp プラグインを更新
cli-agent-plugin-update-warp-extension-step = Warp 拡張機能を更新
cli-agent-plugin-gemini-restart-update-note = 更新を有効化するため Gemini CLI を再起動します。
cli-agent-plugin-opencode-install-title = OpenCode 用 Warp プラグインをインストール
cli-agent-plugin-opencode-install-subtitle = OpenCode の設定に Warp プラグインを追加し、OpenCode を再起動してください。
cli-agent-plugin-opencode-open-config-step = opencode.json を開くか作成します。プロジェクトルートまたはグローバル設定パスに配置できます:
cli-agent-plugin-opencode-add-plugin-step = トップレベル JSON オブジェクトの "plugin" 配列に "@warp-dot-dev/opencode-warp" を追加します:
cli-agent-plugin-opencode-restart-note = プラグインを有効化するため OpenCode を再起動します。
cli-agent-plugin-opencode-update-title = OpenCode 用 Warp プラグインを更新
cli-agent-plugin-opencode-update-subtitle = opencode.json でプラグインを最新バージョンにピン留めします。OpenCode はバージョン指定ごとにプラグインをキャッシュするため、ピンを変更すると再起動時に再取得されます。
cli-agent-plugin-opencode-replace-plugin-step = "plugin" 配列の既存の "@warp-dot-dev/opencode-warp" エントリを明示的なバージョン指定に置き換えます:
cli-agent-plugin-opencode-restart-update-note = 更新されたプラグインを読み込むため OpenCode を再起動します。

# Remaining visible UI strings
ai-ask-user-questions-unavailable = 質問を利用できません
ai-ask-user-questions-skipped-auto-approve = 自動承認のため質問をスキップしました
terminal-bootstrapping-checking = 確認中...
terminal-bootstrapping-installing-progress = インストール中... ({ $p }%)
terminal-bootstrapping-installing = インストール中...
terminal-bootstrapping-updating = 更新中...
terminal-bootstrapping-initializing = 初期化中...
terminal-bootstrapping-installing-warp-ssh-extension-progress = Warp SSH 拡張をインストール中... ({ $p }%)
terminal-bootstrapping-installing-warp-ssh-extension = Warp SSH 拡張をインストール中...
terminal-bootstrapping-updating-warp-ssh-extension = Warp SSH 拡張を更新中...
terminal-bootstrapping-starting-shell-name = { $shell } を起動中...
agent-tip-prefix = ヒント:
agent-tip-slash-menu = `/` でスラッシュコマンドメニューを開き、エージェントのクイックアクションにアクセスできます。
agent-tip-toggle-input-mode = <keybinding> で自然言語検出を切り替え、エージェント入力とターミナル入力を切り替えられます。
agent-tip-plan = `/plan` <prompt> で実行前にエージェント用のプランを作成できます。
agent-tip-command-palette = <keybinding> でコマンドパレットを開き、Warp のアクションやショートカットにアクセスできます。
agent-tip-warp-drive = 再利用可能なワークフロー、ノートブック、プロンプトを保存する場所:
agent-tip-redirect-running-agent = 新しいプロンプトを入力すると、実行中のエージェントを別方向に向け直せます。
agent-tip-add-context = `@` でファイル、ブロック、Warp Drive オブジェクトからコンテキストをプロンプトに追加できます。
agent-tip-attach-prior-output = <keybinding> で直前のコマンド出力をエージェントのコンテキストとして添付できます。
agent-tip-init-index = `/init` でリポジトリをインデックス化し、エージェントがコードベースを理解できるようにします。
agent-tip-agent-profiles = エージェントプロファイルを追加して、セッションごとに権限とモデルをカスタマイズできます。
agent-tip-fork-block = ブロックを右クリックすると、その地点から会話を分岐できます。
agent-tip-copy-output = ブロックを右クリックすると、会話の出力をコピーできます。
agent-tip-drag-image = 画像をペインにドラッグするとエージェントのコンテキストとして添付できます。
agent-tip-interactive-tools = node、python、postgres、gdb、vim などのインタラクティブツールをエージェントに操作させることができます。
agent-tip-code-review-panel = <keybinding> でコードレビューパネルを開き、エージェントの変更をレビューできます。
agent-tip-add-mcp = `/add-mcp` でワークスペースに MCP サーバーを追加できます。
agent-tip-open-mcp-servers = `/open-mcp-servers` で MCP サーバーを表示しチームと共有できます。
agent-tip-create-environment = `/create-environment` でリポジトリをエージェント実行用のリモート Docker 環境に変換できます。
agent-tip-add-prompt = `/add-prompt` で繰り返し可能なワークフロー用の再利用可能なプロンプトを作成できます。
agent-tip-add-rule = `/add-rule` でグローバルなエージェントルールを作成できます。
agent-tip-fork = `/fork` で現在の会話を新しいプロンプト付き(任意)で複製できます。
agent-tip-open-code-review = `/open-code-review` でコードレビューパネルを開き、エージェントが生成した差分を確認できます。
agent-tip-new-conversation = `/new` でクリーンなコンテキストで新しいエージェント会話を開始できます。
agent-tip-compact = `/compact` で現在の会話を要約しコンテキストウィンドウの空きを確保できます。
agent-tip-usage = `/usage` で現在の AI クレジット使用量を表示できます。
agent-tip-oz-headless = `oz` コマンドで Oz エージェントをヘッドレスモードで実行できます。リモートマシンに便利です。
agent-tip-selected-text-context = 選択したテキストを右クリックするとエージェントのコンテキストとして添付できます。
agent-tip-project-rules = `AGENTS.md` または `CLAUDE.md` でプロジェクトスコープのルールを適用できます。
agent-tip-url-context = URL を貼り付けるとそのウェブページをエージェントのコンテキストとして添付できます。
agent-tip-warpify-ssh = リモート SSH セッションを Warpify するとその環境内で Oz を有効化できます。
agent-tip-switch-profiles = エージェントプロファイルを切り替えるとモデルと権限を素早く変更できます。
agent-tip-init-rules = `/init` で `WARP.md` ファイルを生成しエージェント用のプロジェクトルールを定義できます。
agent-tip-auto-approve = <keybinding> で残りのセッションのエージェントコマンドと差分を自動承認できます。
agent-tip-desktop-notifications = デスクトップ通知を有効にするとエージェントが注意を必要とするときに通知を受け取れます。
agent-tip-cancel-task = <keybinding> で現在のエージェントタスクをキャンセルできます。
agent-tip-action-open-palette = パレットを開く
agent-tip-action-warp-drive = Warp Drive。
agent-tip-action-show-diff-view = 差分ビューを表示
agent-tip-voice-input = <keybinding> を押し続けるとプロンプトをエージェントに直接話しかけられます。
hoa-welcome-banner-title = ユニバーサルエージェントサポートのご紹介: Warp であらゆるコーディングエージェントをレベルアップ
hoa-feature-vertical-tabs-title = 縦型タブ
hoa-feature-vertical-tabs-description = git ブランチ、worktree、PR などのリッチなタブタイトルとメタデータ。完全カスタマイズ可能。
hoa-feature-tab-configs-title = タブ設定
hoa-feature-tab-configs-description = タブレベルのスキーマでディレクトリ、起動コマンド、テーマ、worktree をワンクリックで設定
hoa-feature-agent-inbox-title = エージェント受信箱
hoa-feature-agent-inbox-description = エージェントが注意を必要とする際の通知。中央受信箱からもアクセス可能
hoa-feature-native-code-review-title = ネイティブコードレビュー
hoa-feature-native-code-review-description = Warp のコードレビューからインラインコメントを Claude Code、Codex、OpenCode へ直接送信
resource-center-whats-new-section = 新着情報
resource-center-getting-started-section = はじめに
resource-center-maximize-warp-section = Warp を最大限に活用
resource-center-advanced-setup-section = 高度なセットアップ
resource-center-create-first-block-title = 最初のブロックを作成
resource-center-create-first-block-description = コマンドを実行するとコマンドと出力がグループ化されて表示されます。
resource-center-navigate-blocks-title = ブロック間を移動
resource-center-navigate-blocks-description = クリックでブロックを選択し、矢印キーで移動できます。
resource-center-block-action-title = ブロックに対してアクション
resource-center-block-action-description = ブロックを右クリックでコピー/貼り付け、共有などが可能です。
resource-center-command-palette-title = コマンドパレットを開く
resource-center-command-palette-description = キーボードから Warp のすべての機能にアクセス。
resource-center-set-theme-title = テーマを設定
resource-center-set-theme-description = テーマを選んで Warp を自分好みに。
resource-center-custom-prompt-title = カスタムプロンプトを使用
resource-center-custom-prompt-description = PS1 設定を尊重するよう Warp をセットアップ
resource-center-view-documentation = ドキュメントを表示
resource-center-integrate-ide-title = Warp を IDE と連携
resource-center-integrate-ide-description = よく使う開発ツールから Warp を起動できるよう設定
resource-center-how-warp-uses-warp-title = Warp チームの Warp 活用法
resource-center-how-warp-uses-warp-description = Warp のエンジニアリングチームがお気に入り機能をどう使っているか学びます
resource-center-read-article = 記事を読む
resource-center-command-search-title = コマンド検索
resource-center-command-search-description = 過去に実行したコマンド、ワークフローなどを検索して実行。
resource-center-ai-command-search-title = AI コマンド検索
resource-center-ai-command-search-description = 自然言語でシェルコマンドを生成。
resource-center-split-panes-title = ペイン分割
resource-center-split-panes-description = タブを複数のペインに分割し理想的なレイアウトを構築。
resource-center-launch-configuration-title = 起動構成
resource-center-launch-configuration-description = 現在のウィンドウ、タブ、ペイン構成を保存。
notebook-link-new-session = 新しいセッション
notebook-link-new-session-tooltip = このディレクトリで新しいターミナルセッションを開く
notebook-link-open-terminal-session = ターミナルセッションで開く
notebook-link-open-in-editor = エディタで開く
notebook-link-edit-markdown-file = Markdown ファイルを編集
auth-token-placeholder = 認証トークン
sharing-inherited-from-prefix = 継承元: {" "}
sharing-inherited-permission-label = 継承された権限
sharing-inherited-permissions-edit-parent-tooltip = 親フォルダで継承権限を編集
sharing-inherited-permissions-cannot-edit-tooltip = 継承された権限は編集できません
command-palette-navigation-running = 実行中...
command-palette-navigation-completed-over-hour = 1 時間以上前に完了
command-palette-navigation-completed-minute-ago = { $mins } 分前に完了
command-palette-navigation-completed-minutes-ago = { $mins } 分前に完了
command-palette-navigation-no-timestamp = タイムスタンプが見つかりません
command-palette-navigation-completed = 完了
command-palette-navigation-empty-session = 空のセッション
terminal-history-tab-commands = コマンド
terminal-history-tab-prompts = プロンプト
common-current = 現在
auth-browser-token-placeholder = ブラウザ認証トークン
requested-script-expand-to-show = 展開してスクリプトを表示
common-hide = 非表示
terminal-message-new-conversation = {" "}新しい会話
agent-message-bar-again-send-to-agent = 再度押すとエージェントに送信
# =============================================================================
# SECTION: remaining-ui-surfaces (Owner: codex-i18n-remaining-ui-surfaces)
# Files: onboarding slides, auth modal, voice, launch configs, notebook file state,
#        resource center, theme picker, terminal banners, AI footer/tool output
# =============================================================================

onboarding-intention-title = Warp へようこそ
onboarding-intention-subtitle = どのように作業しますか？
onboarding-intention-agent-title = AI エージェントでより速く開発する
onboarding-intention-agent-description = クラス最高のターミナルサポートを備えたエージェントファースト体験。次のようなターミナル/エージェント駆動の AI 機能を利用できます:
onboarding-intention-terminal-title = ターミナルとして使う
onboarding-intention-terminal-badge = AI 機能なし
onboarding-intention-terminal-description = AI を使わず、速度・コンテキスト・コントロールに最適化されたモダンなターミナル。
onboarding-ai-feature-warp-agents = Warp エージェント
onboarding-ai-feature-oz-cloud-agents-platform = Oz ローカルエージェントプラットフォーム
onboarding-ai-feature-next-command-predictions = 次コマンド予測
onboarding-ai-feature-prompt-suggestions = プロンプト候補
onboarding-ai-feature-remote-control-agents = Claude Code、Codex、その他エージェントによるリモートコントロール
onboarding-ai-feature-agents-over-ssh = SSH 経由のエージェント
onboarding-agent-title = Warp エージェントをカスタマイズ
onboarding-agent-subtitle = アプリ内エージェントの既定値を選択します。
onboarding-agent-default-model = 既定モデル
onboarding-agent-autonomy = 自律性
onboarding-agent-set-by-team-workspace = チームワークスペースで設定済み
onboarding-agent-team-workspace-autonomy-description = 自律性の設定はチームワークスペースの一部として構成されています。
onboarding-agent-autonomy-full-title = フル
onboarding-agent-autonomy-full-subtitle = 確認なしでコマンド実行・コード記述・ファイル読み取りを行います。
onboarding-agent-autonomy-partial-title = パーシャル
onboarding-agent-autonomy-partial-subtitle = 計画立案・ファイル読み取り・低リスクコマンドの実行が可能です。変更や機微なコマンド実行の前に確認します。
onboarding-agent-autonomy-none-title = なし
onboarding-agent-autonomy-none-subtitle = 承認なしでは何もアクションを行いません。
onboarding-agent-disable-warp-agent = Warp エージェントを無効化
onboarding-agent-upgrade-title = アップグレードでプレミアムモデルにアクセス。
onboarding-agent-upgrade-subtitle = 最先端モデルには有料プランが必要です。
onboarding-agent-paste-token-link = ここをクリック
onboarding-agent-open-page-manually = {" "}し、手動でページを開いてください。{" "}
onboarding-agent-paste-token-suffix = {" "}してブラウザからトークンを貼り付けます。
onboarding-agent-plan-activated = プランを有効化しました。すべてのプレミアムモデルが利用可能です。
onboarding-project-title = プロジェクトを開く
onboarding-project-subtitle = Warp でのコーディング向けにプロジェクトを設定します。
onboarding-project-open-local-folder = ローカルフォルダを開く
onboarding-project-initialize-automatically = プロジェクトを自動で初期化
onboarding-project-initialize-description = プロジェクト環境を準備し、コードのインデックスを構築し、プロジェクトルールを生成します。エージェントの理解を深め、性能を高めます。
onboarding-intro-already-have-account = すでにアカウントをお持ちですか？{" "}
onboarding-intro-subtitle = 最先端のエージェントを内蔵したモダンなターミナル。
onboarding-get-started = はじめる
onboarding-theme-title = テーマを選択
onboarding-theme-subtitle = クリックまたは矢印キーで選択し、Enter で確定します。
onboarding-theme-sync-with-os = OS のライト/ダークテーマと同期
onboarding-third-party-title = サードパーティエージェントをカスタマイズ
onboarding-third-party-subtitle = Claude Code、Codex、Gemini などのエージェントを使う際の既定値を選択します。
onboarding-third-party-cli-toolbar = CLI エージェントツールバー
onboarding-third-party-notifications = 通知
onboarding-customize-title = Warp をカスタマイズ
onboarding-customize-subtitle = 機能と UI を自分の作業スタイルに合わせて調整します。
onboarding-customize-tab-styling = タブのスタイル
onboarding-customize-vertical = 縦
onboarding-customize-horizontal = 横
onboarding-customize-conversation-history = 会話履歴
onboarding-customize-file-explorer = ファイルエクスプローラー
onboarding-customize-global-file-search = グローバルファイル検索
onboarding-customize-warp-drive = Warp Drive
onboarding-customize-tools-panel = ツールパネル
onboarding-customize-code-review = コードレビュー
onboarding-free-user-title = はじめましょう。
onboarding-free-user-agent-title = Warp 内蔵エージェントによるエージェント駆動開発
onboarding-free-user-agent-description = Warp 内蔵エージェント Oz でローカルに反復・計画・構築。
onboarding-free-user-terminal-title = サードパーティエージェント対応のクラシックターミナル
onboarding-free-user-terminal-description = サードパーティエージェント (Claude Code、Codex、Gemini CLI) と従来型ターミナルワークフローに対応するモダンターミナル。
onboarding-free-user-subscribe-title = サブスクライブして Warp のエージェント駆動開発を利用しましょう。
onboarding-free-user-subscribe-item-credits = 月 1,500 クレジット
onboarding-free-user-subscribe-item-models = OpenAI、Anthropic、Google のフロンティアモデルへのアクセス
onboarding-free-user-subscribe-item-reload = Reload クレジットおよびボリューム割引へのアクセス
onboarding-free-user-subscribe-item-cloud-agents = 拡張エージェントアクセス
onboarding-free-user-subscribe-item-indexing = 最大規模のコードベースインデックス上限
onboarding-free-user-subscribe-item-drive = 無制限の Warp Drive オブジェクトとコラボレーション
onboarding-free-user-subscribe-item-support = プライベートメールサポート
onboarding-free-user-subscribe-item-cloud-storage = ローカル会話保存

auth-opt-out-line-1 = 分析と AI 機能をオプトアウトしたい場合、
auth-opt-out-line-2-prefix = 次から調整できます:{" "}
auth-privacy-settings-prefix = 分析をオプトアウトしたい場合、次から調整できます:{" "}
auth-privacy-settings-ai-prefix = 分析と AI 機能をオプトアウトしたい場合、次から調整できます:{" "}
auth-privacy-settings = プライバシー設定
auth-terms-prefix = 続行することで、Warp の以下に同意したことになります:{" "}
auth-terms-of-service = 利用規約
auth-log-in = ログイン
auth-paste-token-from-browser = ここをクリックしてブラウザからトークンを貼り付け
auth-login-slide-title-warp-drive = Warp Drive をはじめる
auth-login-slide-title-ai = AI をはじめる
auth-login-slide-subtitle-warp-drive = アカウントを接続して、ノートブック・ワークフローなどをデバイス間で保存・共有します。
auth-login-slide-subtitle-ai = アカウントを接続して、AI による計画・コーディング・自動化を有効にします。
auth-disable-warp-drive = Warp Drive を無効化
auth-disable-ai-features = AI 機能を無効化
auth-enable-warp-drive = Warp Drive を有効化
auth-enable-ai-features = AI 機能を有効化
auth-browser-sign-in-one-line-title = 続行するにはブラウザでサインインしてください
auth-open-page-manually-line-prefix = {" "}し、開いてください
auth-open-page-manually-line-suffix = ページを手動で。
auth-disable-warp-drive-confirm-title = 本当に Warp Drive を無効化しますか？
auth-disable-ai-features-confirm-title = 本当に AI 機能を無効化しますか？
auth-disable-warp-drive-confirm-body = Warp Drive はワークフローやナレッジをデバイス間で保存し、チームと共有できます。続行すると、以下の機能が利用できなくなります:
auth-disable-ai-features-confirm-body = Warp は AI でより便利になります。続行すると、以下の機能はいずれも利用できなくなります:
auth-feature-session-sharing = セッション共有
auth-sign-up = サインアップ
auth-sign-in = サインイン
auth-already-have-account = すでにアカウントをお持ちですか？{" "}
auth-dont-want-sign-in-now = 今はサインインしたくない？{" "}
auth-skip-for-now = 今はスキップ
auth-skip-login-confirm-title = 本当にログインをスキップしますか？
auth-skip-login-confirm-line-1 = 後でサインアップできますが、AI など一部の機能は
auth-skip-login-confirm-line-2-prefix = ログインユーザーのみ利用可能です。{" "}
auth-yes-skip-login = はい、ログインをスキップ
auth-require-login-ai-collaboration = Warp の AI 機能を使用したり他のユーザーとコラボレートするには、アカウントを作成してください。
auth-require-login-drive-limit = Warp Drive にこれ以上オブジェクトを作成するには、アカウントを作成してください。
auth-require-login-share = 共有するには、アカウントを作成してください。
auth-welcome-title = Warp へようこそ！
auth-sign-up-for-warp = Warp にサインアップ
auth-browser-sign-in-title = 続行するにはブラウザで\nサインインしてください
auth-browser-not-launched-prefix = ブラウザが起動していない場合、{" "}
auth-copy-url = URL をコピー
auth-open-page-manually-suffix = し、手動でページを開いてください。

voice-try-input = 音声入力を試す
voice-input-enabled-toast = 音声入力が有効です。`{ $key }` キーを押し続けても音声入力を起動できます (設定 > AI > 音声 で構成)
voice-input-microphone-access-error = 音声入力を開始できませんでした (マイクアクセスを有効にする必要があります)
voice-transcription-disabled-microphone = マイクアクセスが許可されていないため、音声書き起こしは無効です。
voice-transcription = 音声書き起こし
voice-transcription-hold-key = 音声書き起こし (`{ $key }` キーを押し続ける)

get-started-welcome-title = Warp へようこそ
get-started-subtitle = エージェント型開発環境
theme-creator-theme-name = テーマ名
theme-creator-background-color = 背景色
theme-creator-image-subheader = 画像 (.png、.jpg) から抽出した色を基にテーマを自動生成します。
theme-creator-select-image = 画像を選択
theme-creator-selecting-image = 画像を選択中…
theme-creator-select-new-image = 新しい画像を選択
theme-creator-create-theme = テーマを作成
theme-creator-process-image-failed = 選択した画像の処理に失敗しました。別の画像でもう一度お試しください。
theme-chooser-current-description = 現在のテーマを変更します。
theme-chooser-light-description = システムがライトモードのときのテーマを選びます。
theme-chooser-dark-description = システムがダークモードのときのテーマを選びます。
theme-chooser-no-matching-themes = 一致するテーマがありません！
resource-center-keyboard-shortcuts = キーボードショートカット
resource-center-keybindings-essentials = 基本
resource-center-keybindings-blocks = ブロック
resource-center-keybindings-input-editor = 入力エディタ
resource-center-keybindings-terminal = ターミナル
resource-center-keybindings-fundamentals = 基礎

launch-config-save-success-prefix = 次に保存しました:{" "}
launch-config-save-failure-already-exists = 保存に失敗しました。同名の起動構成が既に存在します。
launch-config-save-failure-other = 保存中に問題が発生しました。
launch-config-save-configuration = 構成を保存
launch-config-open-yaml-file = YAML ファイルを開く
launch-config-save-current-configuration = 現在の構成を保存
launch-config-link-to-documentation = ドキュメントへのリンク
launch-config-save-modal-a11y-title = 構成保存モーダル
launch-config-save-modal-a11y-description = 現在のウィンドウ・タブ・ペーン構成を保存するファイル名を入力してください。Enter で起動構成を保存、Esc で構成保存モーダルを終了します。
launch-config-save-description-no-keybinding = 現在のウィンドウ・タブ・ペーン構成をファイルに保存し、再度簡単に開けるようにします。
launch-config-save-description-with-keybinding = 現在のウィンドウ・タブ・ペーン構成をファイルに保存し、{ $keybinding } で再度簡単に開けるようにします。
launch-config-yaml-saved-to-prefix = \nYAML ファイルは次に保存されました:{" "}
notebook-file-could-not-read = { $name } を読み込めませんでした
notebook-file-loading = { $name } を読み込み中…
notebook-file-missing-source = ソースファイルがありません

terminal-shared-session-reconnecting = オフライン、再接続を試みています…
terminal-banner-p10k-supported = Powerlevel10k が Warp に対応しました！{"  "}
terminal-banner-p10k-older-version-prefix = 古い (非対応の) バージョンを実行しているようです。次の手順に従ってください:{" "}
terminal-banner-these-instructions = この手順
terminal-banner-update-latest-suffix = {" "}に従って最新版に更新してください。
terminal-banner-pure-unsupported = Pure はまだ Warp で対応していません。代替として対応プロンプトの利用をご検討ください。{"  "}
terminal-loading-session = セッションを読み込み中…

ai-footer-hide-rich-input = リッチ入力を非表示
ai-footer-choose-environment = 環境を選択
ai-footer-agent-environment = エージェント環境
ai-footer-enable-terminal-command-autodetection = ターミナルコマンドの自動検出を有効化
ai-footer-disable-terminal-command-autodetection = ターミナルコマンドの自動検出を無効化
ai-footer-turn-off-auto-approve-agent-actions = すべてのエージェントアクションの自動承認をオフ
ai-footer-auto-approve-agent-actions-for-task = このタスクのすべてのエージェントアクションを自動承認
ai-footer-start-remote-control = リモートコントロールを開始
ai-footer-login-required-remote-control = /remote-control を使用するにはログインしてください
ai-footer-see-logs-for-details = 詳細はログを参照
ai-footer-plugin-installed-restart-session = Warp プラグインをインストールしました。アクティブ化するにはセッションを再起動してください。
ai-footer-installing-warp-plugin = Warp プラグインをインストール中…
ai-footer-failed-install-warp-plugin = Warp プラグインのインストールに失敗しました
ai-footer-plugin-updated-restart-session = Warp プラグインを更新しました。アクティブ化するにはセッションを再起動してください。
ai-footer-updating-warp-plugin = Warp プラグインを更新中…
ai-footer-failed-update-warp-plugin = Warp プラグインの更新に失敗しました
voice-input-limit-reached = 音声入力の上限に達しました
voice-input-transcription-failed = 音声入力の書き起こしに失敗しました
ai-toolbar-context-chip = コンテキストチップ
ai-toolbar-model-selector = モデルセレクター
ai-toolbar-autodetection = 自動検出
ai-toolbar-voice-input = 音声入力
ai-toolbar-attach-file = ファイル添付
ai-toolbar-context-usage = コンテキスト使用量
ai-toolbar-file-explorer = ファイルエクスプローラー
ai-toolbar-rich-input = リッチ入力
ai-toolbar-fast-forward = 早送り
ai-tool-output-grep-for = 次を grep:{" "}
ai-tool-output-grepping-for = 次を grep 中:{" "}
ai-tool-output-in-path-cancelled = {" "}{ $path } 内をキャンセルしました
ai-tool-output-in-path = {" "}{ $path } 内
ai-tool-output-grep-patterns-cancelled = { $path } 内で以下のパターンの grep をキャンセルしました
ai-tool-output-grep-patterns-queued = { $path } 内で以下のパターンを grep
ai-tool-output-grep-patterns-running = { $path } 内で以下のパターンを grep 中
ai-tool-output-search-files-match = 一致するファイルを検索:{" "}
ai-tool-output-finding-files-match = 一致するファイルを検索中:{" "}
ai-tool-output-file-patterns-cancelled = { $path } 内で以下のパターンに一致するファイル検索をキャンセルしました
ai-tool-output-file-patterns-queued = { $path } 内で以下のパターンに一致するファイルを検索
ai-tool-output-file-patterns-running = { $path } 内で以下のパターンに一致するファイルを検索中
ai-tool-output-listing-messages = メッセージを一覧化
ai-tool-output-grepping-patterns = パターンを grep 中
ai-tool-output-grepping-patterns-with-query = パターンを grep 中: { $query }
ai-tool-output-reading-messages = { $count } 件のメッセージを読み込み中

code-review-discard-uncommitted-changes-title = 未コミットの変更を破棄しますか？
code-review-discard-file-uncommitted-changes-title = ファイルへのすべての未コミット変更を破棄しますか？
code-review-discard-all-changes-title = すべての変更を破棄しますか？
code-review-discard-file-changes-title = ファイルへのすべての変更を破棄しますか？
code-review-discard-uncommitted-changes-description = コミットされていないローカル変更をすべて破棄しようとしています。
code-review-discard-file-uncommitted-changes-description = このファイルを最終コミット版に戻し、ローカル編集を破棄します。
code-review-discard-all-changes-description = コミット済み・未コミットを問わずすべての変更を破棄しようとしています。
code-review-discard-file-main-branch-description = このファイルを main ブランチ版に戻し、コミット済み・未コミットの編集をすべて破棄します。
code-review-discard-file-branch-description = このファイルを { $branch } ブランチ版にリセットし、コミット済み・未コミットの編集をすべて破棄します。
code-review-stash-changes = 変更を stash
code-review-no-changes-to-commit = コミットする変更はありません
code-review-no-git-actions-available = 利用可能な git アクションはありません
command-search-out-of-credits-contact-admin = クレジットが不足しているようです。クレジットを増やすにはチーム管理者にアップグレードを依頼してください。
command-search-out-of-credits-prefix = クレジットが不足しているようです。{" "}
command-search-for-more-credits-suffix = {" "}でクレジットを追加してください。
search-not-visible-to-other-users = 他のユーザーには表示されません
sharing-invite = 招待
sharing-who-has-access = アクセス権を持つユーザー
terminal-shared-session-cancel-request = リクエストをキャンセル
terminal-shared-session-continue-sharing = 共有を続行
settings-import-reset-to-warp-defaults = Warp の既定にリセット
settings-import-type-theme = テーマ
settings-import-type-theme-with-comma = テーマ、
settings-import-type-option-as-meta = Option を Meta として扱う
settings-import-type-mouse-scroll-reporting = マウス/スクロール報告
settings-import-type-font = フォント
settings-import-type-default-shell = 既定のシェル
settings-import-type-working-directory = 作業ディレクトリ
settings-import-type-global-hotkey = グローバルホットキー
settings-import-type-window-dimensions = ウィンドウサイズ
settings-import-type-copy-on-select = 選択時にコピー
settings-import-type-window-opacity = ウィンドウの不透明度
settings-import-type-cursor-blinking = カーソル点滅
settings-import-one-other-setting = その他 1 件の設定
settings-import-other-settings = その他 { $count } 件の設定
workflow-argument-editor-helper = このワークフローの引数を入力し、ターミナルセッションで実行するためにコピーします
workflow-add-environment-variables = 環境変数を追加
workflow-environment-variables = 環境変数
workflow-new-environment-variables = 新しい環境変数
ai-history-completed-successfully = 正常に完了しました
ai-history-pending = 保留中
ai-history-cancelled-by-user = ユーザーによりキャンセル
ai-block-always-allow = 常に許可
ai-cancel-summarization = 要約をキャンセル
ai-continue-summarization = 要約を続行
ai-dont-show-suggested-code-banners-again = 提案コードバナーを今後表示しない
ai-inline-code-diff-no-file-name = ファイル名なし
ai-tool-call-cancelled = ツール呼び出しをキャンセルしました
ai-agent-view-open-in-different-pane = 別のペーンで開く
passive-suggestion-feature-or-bug-label = {1} で機能を実装するかバグを修正する
passive-suggestion-help-feature-or-bug-label = {1} での機能実装やバグ修正を手伝う
passive-suggestion-implement-feature-or-bug-query = {1} で機能を実装するかバグを修正してください。必要な詳細はすべて聞いてください。
passive-suggestion-create-pull-request-query = プルリクエストの作成を手伝ってください。
passive-suggestion-start-new-project-label = 新規プロジェクトの開始を手伝う
passive-suggestion-start-new-project-query = 新規プロジェクトの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-node-project-label = Node.js プロジェクトの開始を手伝う
passive-suggestion-node-project-query = Node.js プロジェクトの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-react-app-label = 新しい React アプリの作成を手伝う
passive-suggestion-react-app-query = {1} という新しい React アプリの作成を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-next-app-label = 新しい Next.js アプリの作成を手伝う
passive-suggestion-next-app-query = {1} という新しい Next.js アプリの作成を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-rust-project-label = {1} 用の Rust プロジェクトの開始を手伝う
passive-suggestion-rust-project-query = {1} 用の Rust プロジェクトの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-poetry-project-label = {1} 用の Poetry プロジェクトの開始を手伝う
passive-suggestion-poetry-project-query = {1} 用の Poetry プロジェクトの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-django-project-label = {1} 用の Django プロジェクトの開始を手伝う
passive-suggestion-django-project-query = {1} 用の Django プロジェクトの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-rails-app-label = {1} 用の Rails アプリの開始を手伝う
passive-suggestion-rails-app-query = {1} 用の Rails アプリの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-gradle-maven-project-label = Gradle/Maven プロジェクトの開始を手伝う
passive-suggestion-gradle-maven-project-query = Gradle/Maven プロジェクトの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-go-project-label = {1} 用の Go プロジェクトの開始を手伝う
passive-suggestion-go-project-query = {1} 用の Go プロジェクトの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-swift-project-label = Swift プロジェクトの開始を手伝う
passive-suggestion-swift-project-query = Swift プロジェクトの開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-terraform-config-label = Terraform 構成の開始を手伝う
passive-suggestion-terraform-config-query = Terraform 構成の開始を手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-prisma-setup-label = このプロジェクトでの Prisma セットアップを手伝う
passive-suggestion-prisma-setup-query = このプロジェクトでの Prisma セットアップを手伝ってください。
passive-suggestion-install-dependencies-query = {1} の依存関係のインストールを手伝ってください。
passive-suggestion-ruby-project-label = 新しい Ruby プロジェクトのセットアップを手伝う
passive-suggestion-ruby-project-query = 新しい Ruby プロジェクトのセットアップを手伝ってください。必要な詳細はすべて聞いてください。
passive-suggestion-modelfile-query = {1} 用の Modelfile セットアップを手伝ってください。
passive-suggestion-kubernetes-utilization-query = クラスターのリソース使用状況の理解を手伝ってください。
passive-suggestion-kubernetes-inspect-query = Kubernetes リソースの調査を手伝ってください。
passive-suggestion-docker-containers-query = 実行中コンテナの管理を手伝ってください。
passive-suggestion-docker-images-query = Docker イメージの管理を手伝ってください。
passive-suggestion-docker-compose-label = Docker Compose で {1} の管理またはトラブルシューティングを手伝う
passive-suggestion-docker-compose-query = Docker Compose で {1} の管理またはトラブルシューティングを手伝ってください。
passive-suggestion-docker-network-query = {1} を使用するようコンテナを構成するのを手伝ってください。
passive-suggestion-vagrant-box-query = Vagrant box {1} のセットアップまたはカスタマイズを手伝ってください。
passive-suggestion-vagrant-up-query = 環境のプロビジョニングまたは Vagrant 起動のトラブルシューティングを手伝ってください。
passive-suggestion-grep-search-query = 複数ファイルにわたる {1} のコード検索を手伝ってください。
passive-suggestion-find-search-query = {1} で複数ファイルにわたるコード検索を手伝ってください。
passive-suggestion-ssh-keygen-query = SSH 鍵の生成手順を案内してください。

# =============================================================================
# SECTION: remaining-ui-surfaces (Owner: agent-i18n-remaining)
# Files: app/src/workspace, app/src/terminal, app/src/code, app/src/notebooks,
#        app/src/ai, app/src/settings_view, app/src/workflows, app/src/view_components
# =============================================================================

common-update = 更新
common-reject = 却下
common-open-link = リンクを開く
common-open-file = ファイルを開く
common-open-folder = フォルダを開く
common-name = 名前
common-rule = ルール
common-skip-for-now = 今はスキップ
common-never = しない
common-save-changes = 変更を保存
common-do-not-show-again = 今後表示しない
common-dont-show-again-with-period = 今後表示しません。
common-refresh = 更新
common-resource-not-found-or-access-denied = リソースが見つからないか、アクセスが拒否されました
workspace-close-session = セッションを閉じる
workspace-auto-reload = 自動再読み込み
workspace-add-new-repo = {" "}+ 新しいリポジトリを追加
workspace-notification-permission-denied-toast = Warp にはデスクトップ通知を送る権限がありません。
workspace-troubleshoot-notifications-link = 通知のトラブルシューティング
workspace-plan-synced-to-warp-drive-toast = プランを Warp Drive に同期しました
workspace-remote-control-link-copied-toast = リモートコントロールリンクをコピーしました。
workspace-update-now = 今すぐ更新
workspace-update-warp = Warp を更新
workspace-app-out-of-date-needs-update = アプリが古く、更新が必要です。
workspace-restart-app-and-update-now = アプリを再起動して今すぐ更新
workspace-sampling-process-toast = プロセスを 3 秒間サンプリング中…
workspace-version-deprecation-banner = アプリが古く、一部機能が想定どおり動作しない可能性があります。直ちに更新してください。
workspace-version-deprecation-without-permissions-banner = 直ちに更新しないと一部の Warp 機能が想定どおり動作しない可能性がありますが、Warp は更新を実行できません。
workspace-new-version-unable-to-update-banner = 新しいバージョンが利用可能ですが、Warp は更新を実行できません。
workspace-unable-to-launch-new-installed-version = Warp はインストール済みの新バージョンを起動できませんでした。
tab-config-session-type = セッションタイプ
terminal-copy-error = エラーをコピー
terminal-authenticate-with-github = GitHub で認証
terminal-create-environment = 環境を作成
terminal-regenerate-agents-file = AGENTS.md ファイルを再生成
terminal-view-index-status = インデックスの状態を表示
terminal-shared-session-request-edit-access = 編集アクセスをリクエスト
terminal-create-team = チームを作成
terminal-warpify-without-tmux = TMUX なしで Warpify
terminal-continue-without-warpification = Warpification なしで続行
terminal-always-install = 常にインストール
terminal-never-install = インストールしない
terminal-ssh-report-issue-prefix = Warp の SSH 安定性向上に取り組んでいます。次の対応をご検討ください:{" "}
terminal-ssh-report-issue-link = issue を起票
terminal-ssh-report-issue-suffix = {" "}し、GitHub に投稿いただくと問題を特定しやすくなります。
terminal-ssh-why-need-tmux = なぜ tmux が必要ですか？
terminal-ssh-file-uploads-title = ファイルアップロード
terminal-ssh-close-upload-session = アップロードセッションを閉じる
terminal-ssh-view-upload-session = アップロードセッションを表示
terminal-reveal-secret = シークレットを表示
terminal-hide-secret = シークレットを非表示
terminal-copy-secret = シークレットをコピー
terminal-tag-agent-for-assistance = サポートのためエージェントをタグ付け
terminal-save-as-workflow-secrets-tooltip = シークレットを含むブロックは保存できません。
terminal-agent-mode-setup-title = このコードベース向けに Warp を最適化しますか？
terminal-agent-mode-setup-description = エージェントにコードベースを理解させ、ルールを生成させて、よりスマートで一貫した応答を引き出しましょう。/init を実行することでいつでも実行できます。
terminal-agent-mode-setup-optimize = 最適化
terminal-no-active-conversation-to-export = エクスポートできるアクティブな会話がありません
terminal-slow-shell-startup-banner-prefix = シェルの起動に時間がかかっているようです…{"  "}
terminal-more-info = 詳細情報
terminal-show-initialization-block = 初期化ブロックを表示
terminal-shell-process-exited = シェルプロセスが終了しました
terminal-shell-process-could-not-start = シェルプロセスを開始できませんでした！
terminal-shell-process-exited-prematurely = シェルプロセスが途中で終了しました！
terminal-shell-premature-subtext = { $shell_detail } の起動と Warpify 中に何らかの問題が発生し、プロセスが終了しました。Warpify スクリプトの出力をここに表示します。原因の特定に役立つ可能性があります。
terminal-file-issue = issue を起票
notifications-banner-troubleshoot = トラブルシューティング
notifications-banner-dismissed-title = このバナーは今後表示しませんが、設定からいつでも通知を有効化できます。
notifications-banner-disabled-title = 通知はオフになっていますが、設定からいつでも有効化できます。
notifications-banner-enable = 有効化
notifications-banner-permissions-accepted-title = 成功！デスクトップ通知を受け取る準備が整いました。
notifications-banner-permissions-denied-title = Warp は通知送信の権限を拒否されました。
notifications-banner-permissions-error-title = 権限のリクエスト中に問題が発生しました。
notifications-banner-allow-permissions-title = 通知のセットアップを完了するため、権限リクエストの「許可」をお忘れなく。
notifications-banner-configure-notifications = 通知を構成
notifications-banner-set-permissions = 権限を設定
ai-edit-api-keys = API キーを編集
ai-manage-privacy-settings = プライバシー設定を管理
ai-block-manage-agent-permissions = エージェント権限を管理
agent-zero-state-cloud-agents-description = ローカルエージェントを使い、エージェントを並列実行し、自律的に動作するエージェントを構築し、このマシン上で状況を確認できます。{" "}
agent-zero-state-visit-docs = ドキュメントを見る
ai-execution-profile-agent-decides = エージェントが判断
ai-execution-profile-always-ask = 常に確認
ai-execution-profile-ask-on-first-write = 最初の書き込み時に確認
ai-execution-profile-never-ask = 確認しない
ai-execution-profile-ask-unless-auto-approve = 自動承認以外は確認
code-accept-and-save = 承認して保存
code-hunk-label = ハンク:
code-discard-this-version = このバージョンを破棄
code-overwrite = 上書き
code-review-send-to-agent = エージェントに送信
code-review-open-pr = PR を開く
code-review-pr-created-toast = PR を作成しました。
code-review-comments-sent-to-agent = コメントをエージェントに送信しました
code-review-could-not-submit-comments = エージェントへのコメント送信に失敗しました
code-review-tooltip-view-changes = 変更を表示
code-review-diffs-local-workspaces-only = 差分はローカルワークスペースでのみ動作します。
code-review-diffs-git-repositories-only = 差分は git リポジトリでのみ動作します。
code-review-diffs-wsl-unsupported = 差分は現在 WSL では動作しません。
code-review-generating-commit-message-placeholder = コミットメッセージを生成中…
code-review-type-commit-message-placeholder = コミットメッセージを入力
code-review-committing-loading = コミット中…
code-review-commit-message-label = コミットメッセージ
code-review-no-non-outdated-comments-to-send = 送信できる最新のコメントがありません
code-review-send-diff-comments-to = 差分コメントを { $label } に送信
code-review-ai-must-be-enabled-to-send-comments = エージェントへコメント送信するには AI を有効にする必要があります
code-review-agent-code-review-requires-ai-credits = エージェントによるコードレビューには AI クレジットが必要です
code-review-all-terminals-are-busy = すべてのターミナルが使用中です
code-review-send-diff-comments-to-agent = 差分コメントをエージェントに送信
code-failed-to-load-file-toast = ファイルの読み込みに失敗しました。
code-failed-to-save-file-toast = ファイルの保存に失敗しました。
code-file-saved-toast = ファイルを保存しました。
notebook-apply-link = リンクを適用
notebook-sync-conflict-resolution-message = 編集中に変更が加えられたため、このノートブックを保存できませんでした。作業内容をコピーして再読み込みしてください。
notebook-sync-feature-not-available-message = この機能が一時的に利用できないため、ノートブックをサーバーに保存できませんでした。変更はローカルに保存されています。後でやり直してください。
notebook-link-copied-toast = リンクをコピーしました
settings-share-with-team = チームと共有
tooltip-secrets-not-sent-to-warp-server = *シークレットは Warp のサーバーに送信されません。
editor-voice-limit-hit-toast = 音声リクエストの上限に達しました。次のサイクルの一部として上限が更新されます。
editor-voice-error-toast = 音声入力の処理中にエラーが発生しました。
ai-copied-branch-name-toast = ブランチ名をコピーしました
workflow-new-enum = 新しい enum
workflow-edit-enum = enum を編集
workflow-enum-variant-placeholder = バリアント
workflow-enum-variants = バリアント
quit-warning-dont-save = 保存しない
quit-warning-show-running-processes = 実行中のプロセスを表示
