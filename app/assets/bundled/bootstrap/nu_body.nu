if ($env.WARP_BOOTSTRAPPED? | default "") == "" {
  $env.WARP_USING_WINDOWS_CON_PTY = @@USING_CON_PTY_BOOLEAN@@

  if ($env.WARP_INITIAL_WORKING_DIR? | default "") != "" {
    try { cd $env.WARP_INITIAL_WORKING_DIR } catch { null }
    hide-env WARP_INITIAL_WORKING_DIR
  }

  if ($env.WARP_PATH_APPEND? | default "") != "" {
    let extra_paths = ($env.WARP_PATH_APPEND | split row (char esep) | where {|path| $path != "" })
    if (($env.PATH | describe) | str starts-with "list") {
      $env.PATH = ($env.PATH ++ $extra_paths)
    } else {
      $env.PATH = ([($env.PATH | into string)] ++ $extra_paths | str join (char esep))
    }
    hide-env WARP_PATH_APPEND
  }

  def warp_path_string [] {
    let path = ($env.PATH? | default [])
    if (($path | describe) | str starts-with "list") {
      $path | str join (char esep)
    } else {
      $path | into string
    }
  }

  def warp_command_names_by_type [command_type: string] {
    try {
      scope commands | where type == $command_type | get name | uniq | str join (char nl)
    } catch { "" }
  }

  def warp_linux_distribution [] {
    let os_release_file = if ("/etc/os-release" | path exists) {
      "/etc/os-release"
    } else if ("/usr/lib/os-release" | path exists) {
      "/usr/lib/os-release"
    } else {
      ""
    }

    if $os_release_file == "" {
      ""
    } else {
      try {
        open $os_release_file
        | lines
        | where {|line| $line | str starts-with "NAME=" }
        | first
        | str replace -r '^NAME="?(.*?)"?$' '$1'
      } catch { "" }
    }
  }

  def warp_send_json_message [message: record] {
    let encoded_message = ($message | to json -r | encode hex)
    if ($env.WARP_USING_WINDOWS_CON_PTY? | default false) {
      print -n $"\u{1b}]9278;d;($encoded_message)\a"
    } else {
      print -n $"\u{1b}P$d($encoded_message)\u{1b}\\"
    }
  }

  def warp_send_reset_grid_osc [] {
    if ($env.WARP_USING_WINDOWS_CON_PTY? | default false) {
      print -n "\u{1b}]9279\a"
    }
  }

  def warp_send_generator_output_osc [message: string] {
    let hex_encoded_message = ($message | encode hex)
    let byte_count = ($hex_encoded_message | str length)
    print -n $"\u{1b}]9277;A\a($byte_count);($hex_encoded_message)\u{1b}]9277;B\a"
    warp_send_reset_grid_osc
  }

  def --env warp_run_generator_command [command_id: string, command_text: string] {
    $env._WARP_GENERATOR_COMMAND = "1"
    let result = (try { ^$nu.current-exe -c $command_text | complete } catch { { stdout: "", stderr: ($in | into string), exit_code: 1 } })
    let raw_output = ([$result.stdout $result.stderr] | where {|part| ($part | into string) != "" } | str join (char nl))
    warp_send_generator_output_osc $"($command_id);($raw_output);($result.exit_code)"
  }

  def warp_preexec [] {
    let command_text = (try { commandline } catch { "" })
    warp_send_json_message { hook: "Preexec", value: { command: $command_text } }
    warp_send_reset_grid_osc
  }

  def --env warp_precmd [] {
    if ($env._WARP_SUPPRESS_NEXT_PRECMD? | default "") != "" {
      hide-env _WARP_SUPPRESS_NEXT_PRECMD
      return
    }

    let exit_code = ($env.LAST_EXIT_CODE? | default 0)
    let next_block_id = $"precmd-($env.WARP_SESSION_ID)-(random int 0..2147483647)"
    warp_send_json_message { hook: "CommandFinished", value: { exit_code: $exit_code, next_block_id: $next_block_id } }
    warp_send_reset_grid_osc

    if ($env._WARP_GENERATOR_COMMAND? | default "") != "" {
      hide-env _WARP_GENERATOR_COMMAND
      warp_send_json_message { hook: "Precmd", value: { pwd: "", ps1: "", git_head: "", git_branch: "", virtual_env: "", conda_env: "", node_version: "", session_id: ($env.WARP_SESSION_ID | into int), is_after_in_band_command: true } }
      return
    }

    let git_branch = (try { ^git symbolic-ref --short HEAD err> /dev/null | str trim } catch { "" })
    let git_head = if $git_branch != "" { $git_branch } else { try { ^git rev-parse --short HEAD err> /dev/null | str trim } catch { "" } }
    let honor_ps1 = (($env.WARP_HONOR_PS1? | default "0") == "1")
    warp_send_json_message { hook: "Precmd", value: { pwd: (pwd), ps1: "", honor_ps1: $honor_ps1, rprompt: "", git_head: $git_head, git_branch: $git_branch, virtual_env: ($env.VIRTUAL_ENV? | default ""), conda_env: ($env.CONDA_DEFAULT_ENV? | default ""), node_version: "", kube_config: ($env.KUBECONFIG? | default ""), session_id: ($env.WARP_SESSION_ID | into int) } }
  }

  def warp_report_input [] {
    let input_buffer = (try { commandline } catch { "" })
    warp_send_json_message { hook: "InputBuffer", value: { buffer: $input_buffer } }
    try { commandline edit "" } catch { null }
  }

  def warp_finish_update [update_id: string] {
    warp_send_json_message { hook: "FinishUpdate", value: { update_id: $update_id } }
  }

  def warp_handle_dist_upgrade [source_file_name: string] {
    let apt_config = (try { which apt-config | get path | first } catch { "" })
    if $apt_config == "" { return }
    let apt_sources_dir = (try { ^sh -c $"eval $\((^($apt_config) shell APT_SOURCESDIR 'Dir::Etc::sourceparts/d')\); printf %s $APT_SOURCESDIR" } catch { "" })
    if $apt_sources_dir == "" { return }
    let source_file_path = $"($apt_sources_dir)($source_file_name)"
    if not ($"($source_file_path).list" | path exists) and not ($"($source_file_path).sources" | path exists) and ($"($source_file_path).list.distUpgrade" | path exists) {
      print $"Executing: sudo cp \"($source_file_path).list.distUpgrade\" \"($source_file_path).list\""
      sudo cp $"($source_file_path).list.distUpgrade" $"($source_file_path).list"
    }
  }

  def clear [] {
    warp_send_json_message { hook: "Clear", value: {} }
  }

  def --env warp_change_prompt_modes_to_ps1 [] {
    $env.WARP_HONOR_PS1 = "1"
    warp_set_prompt_indicators
  }

  def --env warp_change_prompt_modes_to_warp_prompt [] {
    $env.WARP_HONOR_PS1 = "0"
    warp_set_prompt_indicators
  }

  def warp_bootstrapped [] {
    let history_format = ($env.config.history.file_format? | default "plaintext")
    let histfile = if $history_format == "plaintext" { $nu.history-path } else { "" }
    let alias_lines = (try { scope aliases | each {|alias| $"($alias.name)\t($alias.expansion? | default "")" } | str join (char nl) } catch { "" })
    let env_var_names = (try { $env | columns | str join (char nl) } catch { "" })
    let os_name = ($nu.os-info.name? | default "")
    let os_category = if $os_name == "macos" { "MacOS" } else if $os_name == "linux" { "Linux" } else if $os_name == "windows" { "Windows" } else { "" }
    let linux_distribution = if $os_category == "Linux" { warp_linux_distribution } else { "" }
    let vi_mode_enabled = if (($env.config.edit_mode? | default "") == "vi") { "1" } else { "" }
    warp_send_json_message { hook: "Bootstrapped", value: { histfile: $histfile, shell: "nu", home_dir: ($nu.home-path? | default ($env.HOME? | default "")), path: (warp_path_string), editor: ($env.EDITOR? | default ""), abbreviations: "", aliases: $alias_lines, function_names: (warp_command_names_by_type "custom"), env_var_names: $env_var_names, builtins: (warp_command_names_by_type "built-in"), keywords: (warp_command_names_by_type "keyword"), shell_version: (version | get version), shell_options: "", rcfiles_start_time: "", rcfiles_end_time: "", shell_plugins: "", vi_mode_enabled: $vi_mode_enabled, os_category: $os_category, linux_distribution: $linux_distribution, wsl_name: ($env.WSL_DISTRO_NAME? | default ""), shell_path: $nu.current-exe } }
  }

  let warp_original_prompt_command = ($env.PROMPT_COMMAND? | default null)
  let warp_original_prompt_command_right = ($env.PROMPT_COMMAND_RIGHT? | default null)
  $env.WARP_ORIGINAL_PROMPT_INDICATOR = ($env.PROMPT_INDICATOR? | default "> ")
  $env.WARP_ORIGINAL_PROMPT_INDICATOR_VI_INSERT = ($env.PROMPT_INDICATOR_VI_INSERT? | default ": ")
  $env.WARP_ORIGINAL_PROMPT_INDICATOR_VI_NORMAL = ($env.PROMPT_INDICATOR_VI_NORMAL? | default "> ")
  $env.WARP_ORIGINAL_PROMPT_MULTILINE_INDICATOR = ($env.PROMPT_MULTILINE_INDICATOR? | default "::: ")

  def --env warp_set_prompt_indicators [] {
    if (($env.WARP_HONOR_PS1? | default "0") == "1") {
      $env.PROMPT_INDICATOR = ($env.WARP_ORIGINAL_PROMPT_INDICATOR? | default "> ")
      $env.PROMPT_INDICATOR_VI_INSERT = ($env.WARP_ORIGINAL_PROMPT_INDICATOR_VI_INSERT? | default ": ")
      $env.PROMPT_INDICATOR_VI_NORMAL = ($env.WARP_ORIGINAL_PROMPT_INDICATOR_VI_NORMAL? | default "> ")
      $env.PROMPT_MULTILINE_INDICATOR = ($env.WARP_ORIGINAL_PROMPT_MULTILINE_INDICATOR? | default "::: ")
    } else {
      $env.PROMPT_INDICATOR = ""
      $env.PROMPT_INDICATOR_VI_INSERT = ""
      $env.PROMPT_INDICATOR_VI_NORMAL = ""
      $env.PROMPT_MULTILINE_INDICATOR = ""
    }
  }

  $env.PROMPT_COMMAND = {||
    let prompt = if (($env.WARP_HONOR_PS1? | default "0") == "1") {
      if (($warp_original_prompt_command | describe) == "closure") {
        do $warp_original_prompt_command
      } else if $warp_original_prompt_command == null {
        "> "
      } else {
        $warp_original_prompt_command | into string
      }
    } else { "" }
    $"\u{1b}]133;A\a($prompt)\u{1b}]133;B\a"
  }

  $env.PROMPT_COMMAND_RIGHT = {||
    let prompt = if (($env.WARP_HONOR_PS1? | default "0") == "1") {
      if (($warp_original_prompt_command_right | describe) == "closure") {
        do $warp_original_prompt_command_right
      } else if $warp_original_prompt_command_right == null {
        ""
      } else {
        $warp_original_prompt_command_right | into string
      }
    } else { "" }
    if $prompt == "" { "" } else { $"\u{1b}]133;P;k=r\a($prompt)\u{1b}]133;B\a" }
  }

  $env.config = (
    $env.config
    | upsert shell_integration.osc133 false
    | upsert shell_integration.osc633 false
    | upsert hooks.pre_execution ([{|| warp_preexec }] ++ ($env.config.hooks.pre_execution? | default []))
    | upsert hooks.pre_prompt ([{|| warp_precmd }] ++ ($env.config.hooks.pre_prompt? | default []))
    | upsert keybindings (($env.config.keybindings? | default []) ++ [
      { name: warp_clear_commandline, modifier: control, keycode: char_p, mode: [emacs vi_normal vi_insert], event: { edit: Clear } }
      { name: warp_report_input, modifier: alt, keycode: char_i, mode: [emacs vi_normal vi_insert], event: { send: ExecuteHostCommand, cmd: "warp_report_input" } }
      { name: warp_prompt_ps1, modifier: alt, keycode: char_p, mode: [emacs vi_normal vi_insert], event: { send: ExecuteHostCommand, cmd: "warp_change_prompt_modes_to_ps1" } }
      { name: warp_prompt_warp, modifier: alt, keycode: char_w, mode: [emacs vi_normal vi_insert], event: { send: ExecuteHostCommand, cmd: "warp_change_prompt_modes_to_warp_prompt" } }
    ])
  )

  warp_set_prompt_indicators
  warp_precmd
  warp_bootstrapped
  $env.WARP_BOOTSTRAPPED = "1"
  $env._WARP_SUPPRESS_NEXT_PRECMD = "1"
}
