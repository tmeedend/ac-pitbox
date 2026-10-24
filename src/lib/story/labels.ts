// The studio's names for node kinds, links and content types, as i18n keys
// written out in full - so the key check (`check-conventions.mjs`) sees every
// one of them.
import type { LinkRole, NodeKind } from "./flow";
import type { ContentType } from "./studio";

export const KIND_LABEL: Record<NodeKind, string> = {
  beat: "story.studio.kindBeat",
  message: "story.studio.kindMessage",
  event: "story.studio.kindEvent",
  branch: "story.studio.kindBranch",
  action: "story.studio.kindAction",
  side_events: "story.studio.kindSideEvents",
  end: "story.studio.kindEnd",
};

export const LINK_LABEL: Record<LinkRole, string> = {
  next: "story.studio.linkNext",
  failure: "story.studio.linkFailure",
  case: "story.studio.linkCase",
  otherwise: "story.studio.linkOtherwise",
  reply: "story.studio.linkReply",
};

/** A content type's name in the editor (SPEC-story-mode, Content types). */
export const CONTENT_TYPE_LABEL: Record<ContentType, string> = {
  message: "story.content.typeMessage",
  call: "story.content.typeCall",
  mail: "story.content.typeMail",
  article: "story.content.typeArticle",
  social_post: "story.content.typeSocialPost",
  video: "story.content.typeVideo",
  listing: "story.content.typeListing",
  statement: "story.content.typeStatement",
  calendar_entry: "story.content.typeCalendarEntry",
  dialogue: "story.content.typeDialogue",
  illustration: "story.content.typeIllustration",
  document: "story.content.typeDocument",
};
