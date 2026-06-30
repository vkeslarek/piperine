/*
 * Generates an equvalent model for RF inductors
 */

#define PI 3.14159265358979323846264338327950288419716939937511
#define VERSION "1.0 (10 Oct. 2001)"

#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <errno.h>
#include "Macros.h"
#include "Numbers.h"
#include "fracpole.h"

main(argc, argv)
int argc;
char *argv[];
{
double L=0, Cp=0, Rp=0, Rs=0, H=0;
double F1=0, F0=0, A;
char *myname = argv[0], *profile = "dd", *subcktName = "", fileName[BUFSIZ];
int i;
double size=0;
struct fpFracPole *pFP;
enum fpProfile profileCode;
FILE *pF;
extern int errno;
extern char *strerror();

/* read command line arguments */
    for (i = 1; i < argc; i++) {
	if (!strncmp(argv[i], "l=",2))
	    L = atof(argv[i]+2);
	else if (!strncmp(argv[i], "cp=", 3))
	    Cp = atof(argv[i]+3);
	else if (!strncmp(argv[i], "rp=", 3))
	    Rp = atof(argv[i]+3);
	else if (!strncmp(argv[i], "rs=", 3))
	    Rs = atof(argv[i]+3);
	else if (!strncmp(argv[i], "h=", 2))
	    H = atof(argv[i]+2);
	else if (!strncmp(argv[i], "lumps=", 6))
	    size = atof(argv[i]+6);
	else if (!strncmp(argv[i], "profile=", 8))
	    profile = argv[i]+8;
	else if (!strncmp(argv[i], "name=", 5))
	    subcktName = argv[i]+5;
	else if (!strncmp(argv[i], "f0=", 3))
	    F0 = atof(argv[i]+3);
	else if (!strncmp(argv[i], "f1=", 3))
	    F1 = atof(argv[i]+3);
	else if (!strncmp(argv[i], "-V", 2) || !strncmp(argv[i], "-v", 2)) {
	    printf("%s\n", VERSION);
	    exit(0);
	}
	else {
	    if (strncmp(argv[i], "-h",2))
		fprintf(stderr,"%s: unknown argument: %s\n\n", myname, argv[i]);
	    helpMessage(myname);
	    exit(0);
	}
    }

/* Assure all parameters were given. */
    if (L <= 0 || Cp <= 0 || Rp <= 0 || Rs <= 0 || H <= 0) {
	fprintf(stderr, "Error: Parameters name, l, rs, cp, rp, and h must ");
	fprintf(stderr, "all be given and greater than 0.\n");
	fprintf(stderr, "Example: %s name=lossyind ", myname);
	fprintf(stderr, "rp=8 rs=0.001 cp=230e-15 l=2.6e-9 h=704000\n");
	exit(1);
    }
    if (F0 < 0 || F1 < 0) {
	fprintf(stderr, "Error: Parameters f0 and f1 must ");
	fprintf(stderr, "be greater than 0 if given.\n");
	exit(1);
    }

/* Validate user specified profile and convert it to enum. */
    if (!strcmp(profile, "fd"))
	profileCode = fpFD;
    else if (!strcmp(profile, "dd"))
	profileCode = fpDD;
    else if (!strcmp(profile, "df"))
	profileCode = fpDF;
    else if (!strcmp(profile, "ff"))
	profileCode = fpFF;
    else {
	fprintf(stderr, "`%s' not valid value for `profile', ", profile);
	fprintf(stderr, "use either `fd', `dd', `df', or `ff'.");
	exit(1);
    }

/* Calculate frequency bounds of skin effect approximation. */
    if (F0 == 0.0)
	F0 = 2*(Rs*Rs)*(H*H);
    if (F1 <= F0)
	F1 = 1/(2*PI*sqrt(L*Cp));
    if (F1 <= F0)
    {   fprintf(stderr, "Error, F1=%lg is less than F0=%lg\n", F1, F0);
	exit(1);
    }
    A = 1; /* for now */

    pFP = fpRealize( A, -0.5, F0, F1, size, profileCode);
    if (pFP == NULL) {
	fprintf( stderr, "Error: insufficient memory available.");
	exit(1);
    }

/* Output realized circuit to a file in the form of a Spectre subcircuit */
    sprintf(fileName, "%s.scs", subcktName);
    pF = fopen(fileName, "w");
    if (pF == NULL) {
	fprintf( stderr, "Error: cannot write to file `%s'\n   %s.\n",
		 fileName, strerror( errno ));
	exit(1);
    }
    fprintf(pF, "simulator lang=spectre\n\n");

/* Print inductor subcircuit */
    fprintf(pF, "//\n");
    fprintf(pF, "// Lossy inductor model\n");
    fprintf(pF, "//\n");
    fprintf(pF, "subckt %s (1 2)\n", subcktName);
    fprintf(pF, "    parameters scaling=1M gmin=1e-12\n");
    fprintf(pF, "    L  (1 3) inductor l=%lg\n", L);
    fprintf(pF, "    S  (3 4) skin_effect s=1/(sqrt(2*M_PI)*%lg)\n", H);
    fprintf(pF, "    Cp (1 5) capacitor c=%lg\n", Cp);
    fprintf(pF, "    Rp (5 4) resistor r=%lg\n", Rp);
    fprintf(pF, "    Rs (4 2) resistor r=%lg\n\n", Rs);

/* Print skin effect subcircuit */
    fprintf(pF, "    subckt skin_effect (1 2)\n");
    fprintf(pF, "        parameters s=1\n");
    fprintf(pF, "        G1 (1 2 3 0) gyrator r=sqrt(scaling)\n");
    fprintf(pF, "        A1 (3 0) fracpole coef=scaling/s\n\n");

/* Print gyrator subcircuit */
    fprintf(pF, "        // Gyrator used to convert fractional impedance\n");
    fprintf(pF, "        // pole into a fractional admittance pole\n");
    fprintf(pF, "        subckt gyrator (t1 b1 t2 b2)\n");
    fprintf(pF, "            parameters r=1kOhm\n");
    fprintf(pF, "            Gm1 (t1 b1 t2 b2) vccs gm=1/r\n");
    fprintf(pF, "            Gm2 (b2 t2 t1 b1) vccs gm=1/r\n");
    fprintf(pF, "            G1 (t1 b1) resistor r=1/gmin\n");
    fprintf(pF, "            G2 (t2 b2) resistor r=1/gmin\n");
    fprintf(pF, "        ends gyrator\n\n");

/* Print fracpole subcircuit */
    fpPrint(pFP, pF, "fracpole", "coef", "        ");
    fpFree(pFP);

/* Terminate open subcircuit definitions and close file. */
    fprintf(pF, "    ends skin_effect\n");
    fprintf(pF, "ends %s\n", subcktName);
    fclose(pF);

    exit(0);
}



helpMessage(myname)
char *myname;
{
char *intro = "\
%s:\
    Generates a Spectre subcircuit description of a lossy inductor.\n\
    See Coilcraft application note on modeling lossy inductors\n\
    at www.coilcraft.com/models.html for more information.\n\n";

char *usage = "\
Usage:\n\
%s name=<subcktname> l=<val> rs=<val> cp=<val> rp=<val> h=<val> \\\n\
		   [lumps=<val>] [profile=<fd, dd, df, or ff>] \\\n\
		   [f0=<val>] [f1=<val>]\n\
where\n\
    name is the name of the inductor subcircuit\n\
    l is the inductance\n\
    rs is the low frequency resistance (the ESR)\n\
    cp is the shunt parasitic capacitance\n\
    rp is the high frequency resistance\n\
    h is the skin effect parameter\n\
    lumps is number of lumps used the skin effect approximation\n\
	(use lumps < 0 to specify lumps/decade)\n\
    profile is determines whether the extreme critical frequenices\n\
	are poles or zeros\n\
	    `fd' implies flat at low freqs, down-slope at high freqs\n\
	    `dd' implies down-slope at low freqs, down-slope at high freqs\n\
	    `df' implies down-slope at low freqs, flat at high freqs\n\
	    `ff' implies flat at low freqs, flat at high freqs\n\
\n\
To print help message, use '-h', to print version, use '-V'\n";

    fprintf(stderr, intro, myname );
    fprintf(stderr, usage, myname );
}



