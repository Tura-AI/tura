export function ReadonlyRow(props: { label: string; value: string }) {
  return (
    <div class="field-row readonly-row">
      <span>{props.label}</span>
      <code>{props.value}</code>
    </div>
  );
}
