import styled from "@emotion/styled";

const Title = styled.h2`
  font-size: var(--aegis-type-h2-size);
  font-weight: 600;
  margin: var(--aegis-space-lg) 0 var(--aegis-space-md) 0;
  color: var(--aegis-ink);
  border-bottom: 2px solid var(--aegis-accent);
  padding-bottom: var(--aegis-space-xs);
`;

interface SectionTitleProps {
  children: string;
}

export function SectionTitle({ children }: SectionTitleProps) {
  return <Title>{children}</Title>;
}
