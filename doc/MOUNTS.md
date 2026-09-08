# 말 타기

소지한 **Horse Reins(고삐)**를 사용하면 탑승하고, 다시 사용하면 하차한다.
고삐는 소모되지 않으며 Rica가 기본가 5골드에 판매한다. 무게 0.5, 중첩 불가.
`consumable=true`는 인벤토리의 사용 동작을 열기 위한 설정이며 서버의 전용 효과에서 수량을 유지한다.

## 이동과 제한

- 탑승 중 이동 속도는 도보의 2배. 기본 6m/s, 달리기 9m/s이며 기존 배고픔 보정이 적용된다.
- 살아 있고 전투 중이 아닌 지상 야외에서 탑승 가능하다. 건물 내부·던전·깊은 물에서는 탈 수 없다.
- 전투, 사망, 상호작용, 실내·던전·깊은 물 진입 또는 고삐를 잃으면 다음 이동 틱에 자동 하차한다.
  이동하지 않아도 검사한다. 얕은 물 기준은 수심 0.6m 이하이다.
- 기존 길찾기와 충돌을 사용한다. 말로 울타리·벽을 통과할 수 없다.
- 거래에 올린 고삐는 기존 아이템 사용 제한을 따른다.
- 탑승 상태는 세션 상태다. 재접속하면 하차 상태로 시작한다.

## 표시와 동기화

`Player.mounted`와 `PlayerMountChanged`로 본인과 주변 플레이어에게 상태를 전달한다
(프로토콜 63). 서버 이동, 브라우저 예측, 다른 플레이어 보간, agent-client 이동 시간 계산에
동일한 2배 배율을 적용한다.

말의 속도에 따라 idle/walk/run을 전환하고 클립 간 0.2초 블렌딩을 한다.
플레이어는 4초 주기의 작은 호흡·상체 흔들림이 있는 기승 자세를 유지하며 말의 `RideSeat` 위치를 따라간다.
달릴 때는 말의 20프레임 보폭에 맞춰 골반이 뒤·위로 움직였다 돌아온다. 후퇴량은 초기 동작의
50%로 줄였다(여자 로그 최대 약 12cm). 머리 고정과 몸통 길이에 맞춰 상승량·기울기도 함께 줄인다.
달리기 중 기준 높이는 보폭의 평균 등 높이로
안정화한다. 머리를 기준으로 몸통 길이를 유지하도록 골반의 후퇴량과 높이를 함께 계산하고,
허리(`Spine`)에서 상체 전체를 앞으로 기울인다. 등 위쪽(`Spine1`·`Spine2`)을 따로 접지 않는다.
발 위치와 방향을 유지하도록 허벅지·무릎을 풀어서, 골반이 올라갈수록 다리가 조금 펴진다.
머리 위치·시선은 유지하며, 손은 달릴 때 보폭에 맞춰 ±1.5cm, 대기 중 호흡에 맞춰 ±0.4cm 움직인다.
서 있을 때는 손을 최대 20cm 내려 편하게 고삐를 잡고, 출발하면 0.2초에 걸쳐 기존 달리기 자세로 돌아온다.
대기 중 손을 몸 쪽으로 18cm 당겨 팔꿈치가 쭉 펴지지 않고 몸 가까이에서 굽혀지도록 한다.
팔꿈치의 굽힘 방향도 어깨 아래쪽으로 전환해 양옆으로 벌어지지 않도록 한다.
손 보정은 팔이 닿는 범위에서 적용하며 뼈 길이는 바꾸지 않는다.
양손의 손가락과 말 입 양옆을 갈색 고삐 두 가닥으로 연결한다. 가운데가 최대 24cm 처지는
곡선을 매 프레임 갱신해 손과 말 머리 움직임을 따라가며, 하차할 때 제거한다.
기존 호흡은 유지하고, 보폭 주기는 말의 재생 속도를 따른다. 시작·정지는 클립 가중치로 블렌딩한다.
기승 중 손의 무기·방패·횃불과 사람 발자국을 숨긴다. 하차하면 기존 표시와 애니메이션으로 돌아간다.
회전 클립 2개는 GLB에 보존하며 현재는 기존 방향 제어를 사용한다.

## 에셋과 검증

- 말의 출처·CC BY 4.0 크레딧·구간표: [animals.md](assets/animals.md).
- 고삐 모델과 아이콘: [items.md](assets/items.md).
- 캐릭터 기승 포즈: [animation.md](assets/animation.md#riding).
- Blender 비교 장면: `assets/horse/riding-preview.blend`, `assets/horse/riding-preview.png`.
- 브라우저의 실제 PlayerModel로 남녀 기사 탑승·달리기·하차를 확인했다.
- 서버 테스트는 반복 사용·비소모·상태 전파·이동 배율·소유권 상실·탑승 제한·자동 하차를 검사한다.

GLB·Blender 원본은 Hugging Face에 업로드했으며 `assets.lock`에 리비전과 파일 해시를 기록했다.

                                                                             
                                                                             
                           xxxxxx                                            
                          xx    xx                                           
                           xxxxxxx                                           
                              xx                                             
                               xx                                            
                                x                                            
                             xxxx                                            
  hhh                     xxxx  xx                                           
    hhhh               xxx       x                                           
       hhh                       x                                           
         hhh                     xx                                          
            hhhhhhhhhhhhhhhhhhhxxxhhhhhhhhhhhhhhhhhhhhhhhhh                  
                   h       xxxx                           hhhh               
                  h       xx                                 hh              
                  h       xx                                  hh             
                  h        xx                                  hh            
                  h         xx                                   h           
                 hh          x                                   hh          
                 h                                                h          
                hh                                                           
                                                                             
                                                                             
                                                                             
                                                                             
                                                                             
                            xxxxxx                                           
                           xx    xx                                          
                            xxxxxxx                                          
                               xx                                            
                                xxx                                          
                                 xxxx                                        
                              xxxx  xxx                                      
   hhh                     xxxx       xxxx                                   
     hhhh               xxx              xx  ▲                               
        hhh                           xxxx   │                               
          hhh                     xxxxx      ▼                               
             hhhhhhhhhhhhhhhhhhhxxxxxhhhhhhhhhhhhhhhhhhhhhhh                 
                    h         xxxx                         hhhh              
                   h           x                              hh             
                   h           xx                              hh            
                   h            x                               hh           
                   h            x                                 h          
                  hh                                              hh         
                  h                                                h         
                 hh                                                          
                                                                             
                                                                             
