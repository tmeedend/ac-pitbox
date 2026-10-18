// The studio's names for node kinds and links, as i18n keys written out in
// full - so the key check (`check-conventions.mjs`) sees every one of them.
import type { LinkRole, NodeKind } from "./flow";

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
};
