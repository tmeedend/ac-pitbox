---
id: parent_call
type: call
from: parent
---
parent: Hello? I saw you on the news!
parent: Are you all right?
* (fine) [I'm fine, really.]
    parent: Good. Come for dinner on Sunday.
* (tired) [Tired, but happy.]
    ~ effect("rel.parent += 1")
    parent: Rest, then. We're proud of you.
