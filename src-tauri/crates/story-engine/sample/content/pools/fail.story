---
id: fail_01_tease
type: message
from: friend
pool: fail_01
when: ["rel.friend >= warm"]
---
Friend: [line] A teasing line, only between friends.
---
id: fail_01_close
type: message
from: friend
pool: fail_01
when: ["event.race_01.position == 4"]
---
Friend: [line] So close.
---
id: fail_01_family
type: message
from: parent
pool: fail_01
---
Parent: [line] An encouraging line.
---
id: fail_02_friend
type: message
from: friend
pool: fail_02
---
Friend: [line] Next time.
---
id: fail_02_parent
type: message
from: parent
pool: fail_02
---
Parent: [line] Proud of you anyway.
---
id: fail_02_rival
type: social_post
from: rival
pool: fail_02
when: ["rel.rival <= cold"]
---
Rival: [line] As expected.
