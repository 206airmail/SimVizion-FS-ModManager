/**
 * The product emblem in the header: the small version of the SimVizion app icon (the eye and
 * "SV" of `src-tauri/icons/source/app-icon-small.svg`, which the 16–48 px icons are made from).
 */
export function Emblem() {
  return (
    <svg viewBox="0 0 1024 1024" width="28" height="28" aria-hidden="true" focusable="false">
      <path
        fill="var(--sv-panel)"
        d="M231 46h562q185 0 185 185v562q0 185-185 185H231Q46 978 46 793V231Q46 46 231 46Z"
      />
      <path
        fill="var(--sv-teal)"
        fillRule="evenodd"
        d="M55 508C154 370 328 201 512 201S870 370 969 508C870 646 696 815 512 815S154 646 55 508Zm101 0c115-137 241-226 356-226s241 89 356 226c-115 137-241 226-356 226S271 645 156 508Z"
      />
      <g transform="translate(137 147) scale(.755 .72)">
        <path
          fill="var(--sv-lime)"
          d="M349 362h135l-49 80H346q-24 0-27 16-3 16 17 16h62q99 0 82 94-15 93-111 93H198l49-80h125q25 0 28-16 3-16-18-16h-62q-96 0-80-96 16-91 109-91Z"
        />
        <path fill="var(--sv-teal)" d="M518 384h87l42 168 83-168h81L655 665h-77Z" />
      </g>
    </svg>
  );
}
