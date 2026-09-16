<script lang="ts">
  // 글꼴 파일을 앱에 같이 넣는다 — 윈도우에 기본으로 없어서, 설치 안 한 PC에서도 똑같이 보이게.
  // 가변 글꼴 하나(2MB)로 모든 굵기. 조각 나눈 판(dynamic-subset)은 웹에서 필요한 글자만 받을 때
  // 이득이고, 앱에는 어차피 전부 들어가서 오히려 더 크다(3.1MB).
  import 'pretendard/dist/web/variable/pretendardvariable.css';

  let { children } = $props();
</script>

{@render children()}

<style>
  /* 메인 창과 위젯이 같이 쓴다 */
  :global(:root) {
    font-family: 'Pretendard Variable', Pretendard, 'Malgun Gothic', system-ui, sans-serif;
  }
</style>
