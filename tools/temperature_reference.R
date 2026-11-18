# Run the upstream Thermimage converter without installing unrelated dependencies.
# Arguments: upstream raw2temp.R, raw-count text file, parameter CSV, output text file.
args <- commandArgs(trailingOnly = TRUE)
source(args[1])
raw <- scan(args[2], quiet = TRUE)
p <- read.csv(args[3])
temperature <- raw2temp(raw, E = p$E, OD = p$OD, RTemp = p$RTemp,
                      ATemp = p$ATemp, IRWTemp = p$IRWTemp, IRT = p$IRT,
                      RH = p$RH, PR1 = p$PR1, PB = p$PB, PF = p$PF,
                      PO = p$PO, PR2 = p$PR2, ATA1 = p$ATA1,
                      ATA2 = p$ATA2, ATB1 = p$ATB1, ATB2 = p$ATB2,
                      ATX = p$ATX)
options(digits = 17)
write.table(temperature, args[4], row.names = FALSE, col.names = FALSE,
            quote = FALSE)
