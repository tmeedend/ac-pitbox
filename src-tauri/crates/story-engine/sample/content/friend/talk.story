---
id: talk_01
type: dialogue
characters: [friend]
---
friend: [line] Face to face, no phone this time. #expression:happy #position:left
friend: [line] Another race, or a break?
* (another) [Another race.]
    friend: [line] That's the spirit. #expression:happy
* (rest) [A break.]
    ~ effect("rel.friend -= 1")
    friend: [line] Fair enough. #expression:neutral
