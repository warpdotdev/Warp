$env.WARP_SESSION_ID = ((date now | format date "%s") + (random int 0..32767 | into string))
let username = ($env.USER? | default ($env.USERNAME? | default ""))
let hostname = (try { ^hostname | str trim } catch { $nu.hostname? | default "" })
let msg = ({ hook: "InitShell", value: { session_id: ($env.WARP_SESSION_ID | into int), shell: "nu", user: $username, hostname: $hostname } } | to json -r | encode hex)
let using_windows_con_pty = @@USING_CON_PTY_BOOLEAN@@
if $using_windows_con_pty { print -n $"\u{1b}]9278;d;($msg)\a" } else { print -n $"\u{1b}P$d($msg)\u{1b}\\" }
