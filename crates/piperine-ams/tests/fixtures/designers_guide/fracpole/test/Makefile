SP_FLAGS= -f wsfbin

all	: fp ind da

fp	:
	../bin/fp f0=1 f1=1e6 slope=-0.5 lumps=-1.5 profile=dd
	spectre $(SP_FLAGS) +log %C:r.log test-fp.scs

ind	: 
	../bin/ind name=a01t l=2.6e-9 rp=8 rs=0.001 cp=230e-15 h=704000 lumps=-1.5 profile=dd f0=25e3 f1=65e10
	spectre $(SP_FLAGS) +log %C:r.log test-ind.scs

da	:
	../bin/da name=lossycap cinf=10e-9 c0=22.5e-9 tau0=1 alpha=0.75 f0=0.01 f1=1e6 esr=0.25 esl=8e-9 rleak=1e9 lumps=-1.5 profile=fd
	spectre $(SP_FLAGS) +log %C:r.log test-da.scs

clean	:
	rm -rf *.log *.raw a01t.scs fracpole.scs lossycap.scs expected.plt r raw x z
