interface Props {
  image: string | null;
  label: string;
  color?: string | null;
  size?: number;
  round?: boolean;
}

/** An icon or profile picture, falling back to the first letter on a color. */
export default function Avatar({ image, label, color, size = 28, round }: Props) {
  const style = { width: size, height: size, borderRadius: round ? "50%" : size * 0.24 };
  if (image) return <img className="avatar" src={image} alt="" style={style} draggable={false} />;
  return (
    <span className="avatar monogram" style={{ ...style, background: color ?? undefined, fontSize: size * 0.46 }}>
      {label.trim().charAt(0).toUpperCase() || "?"}
    </span>
  );
}
