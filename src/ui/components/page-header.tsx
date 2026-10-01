import styled from "@emotion/styled";

// 页面头（方案 A-1 语系）：h1 页标题 + sub 副标题，右侧动作槽。

const Head = styled.div`
  display: flex;
  align-items: flex-start;
  gap: var(--aegis-space-lg);
  margin-bottom: var(--aegis-space-lg);
`;

export const PageTitle = styled.h1`
  font-size: 24px;
  font-weight: 700;
  line-height: 1.25;
`;

const Sub = styled.div`
  font-size: 13px;
  color: var(--aegis-mute);
  margin-top: 2px;
`;

const Acts = styled.div`
  margin-left: auto;
  display: flex;
  gap: var(--aegis-space-sm);
  padding-top: 6px;
  flex: none;
`;

export function PageHeader({
  title,
  sub,
  actions,
}: {
  title: string;
  sub?: React.ReactNode;
  actions?: React.ReactNode;
}) {
  return (
    <Head>
      <div>
        <PageTitle>{title}</PageTitle>
        {sub ? <Sub>{sub}</Sub> : null}
      </div>
      {actions ? <Acts>{actions}</Acts> : null}
    </Head>
  );
}
