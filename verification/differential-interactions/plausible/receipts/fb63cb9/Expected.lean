import NobleFoundation.Sequence
open NobleFoundation NobleContracts

-- source sha256 581a54d947860b82346513ccf3f297ecba2eb7cfc3db436e1d3eb2d01ad7702c
theorem case_0_ref : refRun ([] ++ [.push (-8936309182246613764), .push (-2), .add]) [] = some [(-8936309182246613766)] := by decide
example (pre t : Stack) := exec_inputs_nested [] [.push (-8936309182246613764), .push (-2), .add] case_0_ref pre t

-- source sha256 d09ba2837408c7a2db685252bcd3a797269425cdc925593d59f7d25cca27a4c7
theorem case_1_ref : refRun ([.push 7380523375172052064] ++ [.push (-3037000500), .add]) [] = some [7380523372135051564] := by decide
example (pre t : Stack) := exec_inputs_nested [.push 7380523375172052064] [.push (-3037000500), .add] case_1_ref pre t

-- source sha256 6b7c88c19d81a9f132082ca21d4c384dff754912a20045bf4afb0607dfe3573f
theorem case_2_ref : refRun ([.push (-3037000500)] ++ PExpr.flat (.compose (.arith [.push (-4611686018427387904), .add]) (.quoted 4294967296))) [] = some [4294967296, (-4611686021464388404)] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.compose (.arith [.push (-4611686018427387904), .add]) (.quoted 4294967296)) [.push (-3037000500)] case_2_ref pre t

-- source sha256 c5fa2dc30ec9672015ecfb9689f41dea8821d7704f920a54b83e35be43a06e6c
theorem case_3_ref : refRun ([] ++ [.push (-3250464868976199841), .push 1559655854772313886, .sub]) [] = some [(-4810120723748513727)] := by decide
example (pre t : Stack) := exec_inputs_nested [] [.push (-3250464868976199841), .push 1559655854772313886, .sub] case_3_ref pre t

-- source sha256 4aa89347074bf8269022d40791cb2285f7b30b050ce0c80cdc8c42f9052cd843
theorem case_4_ref : refRun [.push 9193307235356993321, .push (-3788329303902485732), .push 3037000499, .push 5360535979711393760, .push 5622977308943621380, .push (-8289670426420317633), .sub, .sub, .sub, .mul, .add] [] = some [(-4542246106112673335)] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_4_ref pre t

-- source sha256 8b5166e49be0860166b3b171ccb9bf618f6c2df71f09d497ba908a2c2e1d712e
theorem case_5_ref : refRun [.push (-3037000500), .push 1, .add] [] = some [(-3037000499)] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_5_ref pre t

-- source sha256 c7e8ca01592a39aedd4d54dff4583dabe951e0cdfdf24c6a310920c1a5b5ad05
theorem case_6_ref : refRun [.push 2162077401622541269, .push 2592011508614518740, .push 9223372036854775806, .add, .mul] [] = some [8763148087439484858] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_6_ref pre t

-- source sha256 ea97666507ebf2773eb451c8f16aafdaa76b4f48a116851047416adaffbd4726
theorem case_7_ref : refRun ([.push (-3), .push (-4611686018427387904)] ++ [.push (-3860185971516211225), .mul, .add]) [] = some [4611686018427387901] := by decide
example (pre t : Stack) := exec_inputs_nested [.push (-3), .push (-4611686018427387904)] [.push (-3860185971516211225), .mul, .add] case_7_ref pre t

-- source sha256 074b9147db518b52fe0088e680ee7c69da1f517a60dfbe0c96b6cccdb73bd5dc
theorem case_8_ref : refRun [.push (-3914109182464487665)] [] = some [(-3914109182464487665)] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_8_ref pre t

-- source sha256 99ac264bf9430c0670e0388f91aa3ebee00b95b2f3ed4927e2bbd58b0a4ea555
theorem case_9_ref : refRun ([.push 7254829345931326319] ++ PExpr.flat (.quoted 4300335662125858302)) [] = some [4300335662125858302, 7254829345931326319] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.quoted 4300335662125858302) [.push 7254829345931326319] case_9_ref pre t

-- source sha256 3f5d7b7b5c9a1be86487c557f10da2f7e2a6d94c75fc56647078d826c0508576
theorem case_10_ref : refRun ([] ++ PExpr.flat (.arith [.push 3037000499, .push 5676416417509980765, .mul])) [] = some [(-1970373348930132089)] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.arith [.push 3037000499, .push 5676416417509980765, .mul]) [] case_10_ref pre t

-- source sha256 a5ca204a94e3539ac0e100c1834a6f5c0cccc77198c83f76c3931fac487fa49c
theorem case_11_ref : refRun ([.push 5915290794636541318, .push 7349712707944994343] ++ [.add, .push 6337970843290529344, .sub]) [] = some [6927032659291006317] := by decide
example (pre t : Stack) := exec_inputs_nested [.push 5915290794636541318, .push 7349712707944994343] [.add, .push 6337970843290529344, .sub] case_11_ref pre t

-- source sha256 7ff899035d8a02231c085602ab198e901dbfb9b83269ea482023c9887c38a605
theorem case_12_ref : refRun ([.push 2147483648] ++ PExpr.flat (.compose (.arith [.push (-9223372036854775807), .mul, .push 9223372036854775807, .add]) (.arith [.push 4294967296, .mul]))) [] = some [9223372032559808512] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.compose (.arith [.push (-9223372036854775807), .mul, .push 9223372036854775807, .add]) (.arith [.push 4294967296, .mul])) [.push 2147483648] case_12_ref pre t

-- source sha256 2d84d30b1bfb15635f1962a3ad4abbf47ce74115ea6ae34c9be05e56dac63178
theorem case_13_ref : refRun ([] ++ PExpr.flat (.compose (.arith [.push 8983063487338953604, .push (-3159069410772003191), .sub, .push (-9223372036854775808), .sub]) (.compose (.quoted 2996888570324207050) (.arith [.push 9223372036854775806, .push 4294967295, .mul, .sub, .mul])))) [] = some [(-7591310650433303784)] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.compose (.arith [.push 8983063487338953604, .push (-3159069410772003191), .sub, .push (-9223372036854775808), .sub]) (.compose (.quoted 2996888570324207050) (.arith [.push 9223372036854775806, .push 4294967295, .mul, .sub, .mul]))) [] case_13_ref pre t

-- source sha256 388f43477b4709c0772dc837d62f1f940ccc70adc5afb3122ac0c00ecf0f6054
theorem case_14_ref : refRun ([.push 3, .push (-5782814818432194501)] ++ PExpr.flat (.compose (.compose (.quoted 2) (.quoted 6957004115249272709)) (.compose (.arith [.add, .push 563216646640416076, .push (-440823945720597602), .push (-9223372036854775808), .mul, .sub, .mul, .sub, .sub]) (.arith [.push (-2147483648), .sub])))) [] = some [(-1220378294732224804)] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.compose (.compose (.quoted 2) (.quoted 6957004115249272709)) (.compose (.arith [.add, .push 563216646640416076, .push (-440823945720597602), .push (-9223372036854775808), .mul, .sub, .mul, .sub, .sub]) (.arith [.push (-2147483648), .sub]))) [.push 3, .push (-5782814818432194501)] case_14_ref pre t

-- source sha256 baac9eda975406f8df6792c7a414652bcffc1b26c35508ba874c42758be881ce
theorem case_15_ref : refRun ([] ++ PExpr.flat (.arith [.push 3379940506471610498, .push 3037000499, .mul])) [] = some [5210537574721071078] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.arith [.push 3379940506471610498, .push 3037000499, .mul]) [] case_15_ref pre t

-- source sha256 83acd6ec9b70e1b3d1bf22d3451ebba0ea14c2de3fc46dcc761886521b339147
theorem case_16_refused : refRun [.push 9223372036854775808] [] = none := by decide

-- source sha256 26dccfe262724f3e2a6fb5245844fcec6ebfa55c2b9299ec50fb4df64b3beda6
theorem case_17_refused : refRun [.push 1, .push (-9223372036854775809), .add] [] = none := by decide

-- source sha256 eb4df4aa17f21749af98a5b6056d6c6890b5e4d138b0c6d5e8389fcd0f758d8e
theorem case_18_refused : refRun [.push 5, .push 18446744073709551616, .add] [] = none := by decide

-- source sha256 2f8a1e93775716a43fa1b73212c02debb9a1ab97a6590bece51543dbf828f1d9
theorem case_19_ref : refRun [.push 2, .push 3, .mul, .push 0, .add, .push 0, .add, .push 0, .add] [] = some [6] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_19_ref pre t

-- source sha256 5fab8d6607a286c2e3b6b8a6648301117ea3bac819215d342bc7b650a9e5bb57
theorem case_20_ref : refRun [.push 0, .push (-1), .sub, .push 0, .add] [] = some [1] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_20_ref pre t

-- source sha256 66d48909ad90d81139a2c48cc3b3bb2a008c357b753280a1bb789754502f41c4
theorem case_21_ref : refRun ([.push 9223372036854775807, .push 1] ++ PExpr.flat (.arith [.add, .push 0, .add, .push 0, .add, .push 0, .add])) [] = some [(-9223372036854775808)] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.arith [.add, .push 0, .add, .push 0, .add, .push 0, .add]) [.push 9223372036854775807, .push 1] case_21_ref pre t

-- source sha256 a34443a6ebe39f2f0414ad73ed5f75787abbacdcaa576e5d3933deafd4d69e2c
theorem case_22_ref : refRun ([.push 9223372036854775807] ++ PExpr.flat (.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul, .push 0, .add]))) [] = some [0] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul, .push 0, .add])) [.push 9223372036854775807] case_22_ref pre t

-- source sha256 b1262b35e57e0a76f65c83fe2f62dc44e061cc5946761fdafb679b699dada2db
theorem case_23_ref : refRun [.push 2, .push 3, .mul, .push 0, .add] [] = some [6] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_23_ref pre t

-- source sha256 b224d5272a484345f7be8b6371d734b6553503a1efe8314afd6abd6841e739d9
theorem case_24_ref : refRun [.push 0, .push (-1), .sub, .push 0, .add, .push 0, .add, .push 0, .add] [] = some [1] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_24_ref pre t

-- source sha256 cd94e2c1e16401208cb591ae909fa6ffa9117621f35622410fa62a15606cac6e
theorem case_25_ref : refRun ([.push 9223372036854775807, .push 1] ++ PExpr.flat (.arith [.add, .push 0, .add])) [] = some [(-9223372036854775808)] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.arith [.add, .push 0, .add]) [.push 9223372036854775807, .push 1] case_25_ref pre t

-- source sha256 cfb846006e7c9614ef4bf756d472ca0e8cd456cc94ded8522d4045f228e19729
theorem case_26_ref : refRun ([.push 9223372036854775807] ++ PExpr.flat (.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul, .push 0, .add, .push 0, .add, .push 0, .add]))) [] = some [0] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul, .push 0, .add, .push 0, .add, .push 0, .add])) [.push 9223372036854775807] case_26_ref pre t

-- source sha256 2f8a1e93775716a43fa1b73212c02debb9a1ab97a6590bece51543dbf828f1d9
theorem case_27_ref : refRun [.push 2, .push 3, .mul, .push 0, .add, .push 0, .add, .push 0, .add] [] = some [6] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_27_ref pre t

-- source sha256 5fab8d6607a286c2e3b6b8a6648301117ea3bac819215d342bc7b650a9e5bb57
theorem case_28_ref : refRun [.push 0, .push (-1), .sub, .push 0, .add] [] = some [1] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_28_ref pre t

-- source sha256 66d48909ad90d81139a2c48cc3b3bb2a008c357b753280a1bb789754502f41c4
theorem case_29_ref : refRun ([.push 9223372036854775807, .push 1] ++ PExpr.flat (.arith [.add, .push 0, .add, .push 0, .add, .push 0, .add])) [] = some [(-9223372036854775808)] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.arith [.add, .push 0, .add, .push 0, .add, .push 0, .add]) [.push 9223372036854775807, .push 1] case_29_ref pre t

-- source sha256 a34443a6ebe39f2f0414ad73ed5f75787abbacdcaa576e5d3933deafd4d69e2c
theorem case_30_ref : refRun ([.push 9223372036854775807] ++ PExpr.flat (.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul, .push 0, .add]))) [] = some [0] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul, .push 0, .add])) [.push 9223372036854775807] case_30_ref pre t

-- source sha256 b1262b35e57e0a76f65c83fe2f62dc44e061cc5946761fdafb679b699dada2db
theorem case_31_ref : refRun [.push 2, .push 3, .mul, .push 0, .add] [] = some [6] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_31_ref pre t

-- source sha256 b224d5272a484345f7be8b6371d734b6553503a1efe8314afd6abd6841e739d9
theorem case_32_ref : refRun [.push 0, .push (-1), .sub, .push 0, .add, .push 0, .add, .push 0, .add] [] = some [1] := by decide
example (pre t : Stack) := exec_lowerAll_from_empty case_32_ref pre t

-- source sha256 cd94e2c1e16401208cb591ae909fa6ffa9117621f35622410fa62a15606cac6e
theorem case_33_ref : refRun ([.push 9223372036854775807, .push 1] ++ PExpr.flat (.arith [.add, .push 0, .add])) [] = some [(-9223372036854775808)] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.arith [.add, .push 0, .add]) [.push 9223372036854775807, .push 1] case_33_ref pre t

-- source sha256 cfb846006e7c9614ef4bf756d472ca0e8cd456cc94ded8522d4045f228e19729
theorem case_34_ref : refRun ([.push 9223372036854775807] ++ PExpr.flat (.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul, .push 0, .add, .push 0, .add, .push 0, .add]))) [] = some [0] := by decide
example (pre t : Stack) := PExpr.exec_inputs_run (.compose (.arith [.push 1, .add]) (.arith [.push 2, .mul, .push 0, .add, .push 0, .add, .push 0, .add])) [.push 9223372036854775807] case_34_ref pre t

-- source sha256 83acd6ec9b70e1b3d1bf22d3451ebba0ea14c2de3fc46dcc761886521b339147
theorem case_35_refused : refRun [.push 9223372036854775808] [] = none := by decide

