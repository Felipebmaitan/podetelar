import icon from "../../../assets/PodeTelar.png";

export function BrandMark({ className }: { className: string }) {
  return <img className={className} src={icon} alt="" aria-hidden="true" />;
}

export function CardBrand() {
  return (
    <div className="invitation-brand">
      <BrandMark className="brand-mark" />
      <span>PodeTelar</span>
    </div>
  );
}
