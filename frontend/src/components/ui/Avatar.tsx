import { clsx } from "clsx";
import "./Avatar.css";

interface AvatarProps {
  readonly src: string;
  readonly alt: string;
  readonly size?: 20 | 24 | 28 | 32 | 40 | 48 | 64 | 80;
  readonly live?: boolean;
  readonly className?: string;
}

export function Avatar({ src, alt, size = 32, live = false, className }: AvatarProps) {
  return (
    <span
      className={clsx("ls-avatar", live && "ls-avatar--live", className)}
      style={{ width: size, height: size }}
    >
      <img src={src} alt={alt} width={size} height={size} loading="lazy" />
    </span>
  );
}
