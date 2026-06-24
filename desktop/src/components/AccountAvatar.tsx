import { initials, avatarColorIndex } from "../lib/format";
import "./AccountAvatar.css";

interface Props {
  name: string;
  did: string;
}

// TODO: hexagon frame for bot accounts in Day 3 (isBot from getAccountInfo)
export default function AccountAvatar(props: Props) {
  return (
    <div class={`account-avatar avatar-c${avatarColorIndex(props.did)}`}>
      {initials(props.name)}
    </div>
  );
}
